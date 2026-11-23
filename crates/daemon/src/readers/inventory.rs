//! Runs the probes of the tool inventory (CAP-020 to CAP-028); nothing a probe prints is logged.
//! A probe gets no input and an environment of only `PATH`, `LC_ALL=C` and `RUSTUP_AUTO_INSTALL=0`,
//! which keeps a rustup proxy from downloading a toolchain; the operator's `~/.claude` and
//! `~/.codex` are never read or changed (CAP-025).

use std::env;
#[cfg(unix)]
use std::ffi::OsStr;
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use daemon_core::inventory::{
    Architecture, Inventory, Os, Outcome, PROBES, Program, TARGETS, TOOL_FOLDERS, candidates,
    kernel_release, macos_version, may_run, os_release_version,
};
use tempfile::TempDir;
use tokio::process::Command;

use super::Platform;
use super::command::{Bounds, CommandError, FixedCommand, capture};

#[cfg(all(test, unix))]
mod tests;

/// The bounds of one probe (CAP-022).
const PROBE_BOUNDS: Bounds = Bounds {
    limit: Duration::from_secs(5),
    keep_cut_output: false,
    output_limit: 64 * 1024,
};

const FILE_LIMIT: usize = 64 * 1024;

const SW_VERS: FixedCommand = FixedCommand {
    program: "/usr/bin/sw_vers",
    args: &["-productVersion"],
    limit: Duration::from_secs(2),
    keep_cut_output: false,
};

/// Fails without the developer tools, and does not offer to install them.
const XCODE_SELECT: FixedCommand = FixedCommand {
    program: "/usr/bin/xcode-select",
    args: &["-p"],
    limit: Duration::from_secs(2),
    keep_cut_output: false,
};

struct Search {
    folders: Vec<PathBuf>,
    path: OsString,
    os: Os,
    /// Whether the macOS developer tools are present, so Apple's placeholders may run.
    developer_tools: bool,
}

/// `None` sends nothing, so the hub keeps the inventory it has.
pub async fn inventory(platform: Platform) -> Option<Inventory> {
    let architecture = Architecture::parse(env::consts::ARCH)?;
    let os = match platform {
        Platform::Linux => Os::Linux,
        Platform::Macos => Os::Macos,
        #[cfg(windows)]
        Platform::Windows => Os::Windows,
    };
    let os_version = os_version(os).await?;
    let search = Search::here(os).await;
    let mut found = probe_all((architecture, os, os_version), &search).await;
    found.remote_access = remote_access().await;
    Some(found)
}

impl Search {
    async fn here(os: Os) -> Self {
        let home =
            env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        let folders = candidates(
            env::var_os("PATH").as_deref(),
            home.as_deref(),
            if cfg!(windows) {
                &[
                    "~/.cargo/bin",
                    "~/.bun/bin",
                    "~/.local/bin",
                    "~/AppData/Roaming/npm",
                ]
            } else {
                &TOOL_FOLDERS
            },
        );
        let path = env::join_paths(&folders).unwrap_or_default();
        let developer_tools = os == Os::Macos && developer_tools().await;
        Self {
            folders,
            path,
            os,
            developer_tools,
        }
    }
}

async fn developer_tools() -> bool {
    let Ok(printed) = XCODE_SELECT.run().await else {
        return false;
    };
    #[cfg(unix)]
    let folder = OsStr::from_bytes(printed.trim_ascii_end());
    #[cfg(windows)]
    let folder = std::str::from_utf8(printed.trim_ascii_end()).unwrap_or_default();
    tokio::fs::metadata(folder)
        .await
        .is_ok_and(|found| found.is_dir())
}

async fn probe_all(system: (Architecture, Os, String), search: &Search) -> Inventory {
    let mut outcomes = Vec::new();
    for probe in PROBES.iter().filter(|probe| probe.runs_on(system.1)) {
        outcomes.push((probe, run(probe.program, probe.own_home, search).await));
    }
    let targets = match run(TARGETS, false, search).await {
        Outcome::Printed(output) => Some(output),
        Outcome::Missing | Outcome::TimedOut | Outcome::Exited => None,
    };
    Inventory::new(system, &outcomes, targets.as_deref())
}

async fn os_version(os: Os) -> Option<String> {
    match os {
        Os::Windows => windows_version(),
        Os::Macos => macos_version(&SW_VERS.run().await.ok()?),
        Os::Linux => {
            let release = read_capped(Path::new("/etc/os-release")).await;
            if let Some(version) = release.as_deref().and_then(os_release_version) {
                return Some(version);
            }
            kernel_release(&read_capped(Path::new("/proc/sys/kernel/osrelease")).await?)
        }
    }
}

async fn read_capped(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = tokio::fs::read(path).await.ok()?;
    bytes.truncate(FILE_LIMIT);
    Some(bytes)
}

async fn run(program: Program, own_home: bool, search: &Search) -> Outcome {
    let Some(path) = find(program.name, search).await else {
        return Outcome::Missing;
    };
    let mut command = Command::new(path);
    command
        .args(program.args)
        .env_clear()
        .env("PATH", &search.path)
        .env("LC_ALL", "C")
        .env("RUSTUP_AUTO_INSTALL", "0")
        .current_dir(if cfg!(windows) {
            env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into())
        } else {
            "/".into()
        });
    #[cfg(windows)]
    for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let home = match own_home.then(|| give_home(&mut command)) {
        // A probe with no folder of its own does not run, rather than get the operator's home.
        Some(None) => return Outcome::Exited,
        Some(home) => home,
        None => None,
    };
    let outcome = outcome(capture(&mut command, program.name, PROBE_BOUNDS).await);
    if let Some(home) = home {
        remove(home);
    }
    outcome
}

/// The operator's `~/.claude` and `~/.codex` are not read or changed by a probe (CAP-025).
fn give_home(command: &mut Command) -> Option<TempDir> {
    let home = tempfile::tempdir()
        .inspect_err(|error| tracing::debug!(%error, "no home folder for a tool probe"))
        .ok()?;
    for name in [
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
    ] {
        command.env(name, home.path());
    }
    Some(home)
}

fn remove(home: TempDir) {
    if let Err(error) = home.close() {
        tracing::debug!(%error, "a tool probe's home folder was not removed");
    }
}

fn outcome(captured: Result<Vec<u8>, CommandError>) -> Outcome {
    match captured {
        Ok(output) => Outcome::Printed(output),
        Err(error) => {
            tracing::debug!(%error, "a tool probe gave no version");
            match error {
                CommandError::Timeout { .. } => Outcome::TimedOut,
                CommandError::Start { .. } | CommandError::Failed { .. } => Outcome::Exited,
            }
        }
    }
}

async fn find(name: &str, search: &Search) -> Option<PathBuf> {
    for path in probe_paths(&search.folders, name) {
        if let Ok(found) = tokio::fs::metadata(&path).await
            && found.is_file()
            && executable(&found)
            && may_run(&path, search.os, search.developer_tools)
        {
            return Some(path);
        }
    }
    None
}

fn probe_paths(folders: &[PathBuf], name: &str) -> Vec<PathBuf> {
    folders
        .iter()
        .flat_map(|folder| {
            if cfg!(windows) {
                vec![
                    folder.join(format!("{name}.exe")),
                    folder.join(format!("{name}.cmd")),
                ]
            } else {
                vec![folder.join(name)]
            }
        })
        .collect()
}

fn executable(found: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        found.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        found.is_file()
    }
}

fn windows_version() -> Option<String> {
    #[cfg(windows)]
    {
        sysinfo::System::os_version()
            .ok()
            .filter(|value| daemon_core::inventory::is_version(value))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

async fn remote_access() -> Option<daemon_core::inventory::RemoteAccess> {
    let bytes = read_capped(Path::new("/etc/anima/remote-access.json")).await?;
    let mut found: daemon_core::inventory::RemoteAccess = serde_json::from_slice(&bytes).ok()?;
    if !found.accepted() {
        return None;
    }
    for (port, user) in [(22, &mut found.ssh_user), (3389, &mut found.rdp_user)] {
        if user.is_some()
            && !matches!(
                tokio::time::timeout(
                    Duration::from_secs(1),
                    tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                )
                .await,
                Ok(Ok(_))
            )
        {
            *user = None;
        }
    }
    Some(found)
}
