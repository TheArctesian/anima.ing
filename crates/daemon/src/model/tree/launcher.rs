//! The choice of the launcher of a server on Linux (contract K11 Section 9.1; task L1-6c). Where
//! `AppArmor` restricts unprivileged user namespaces, the daemon program cannot make the
//! namespaces of a server, so the root-owned model launcher starts it, and only a runtime on the
//! kept signed list. Elsewhere the daemon program is the launcher, as before.

use std::path::{Path, PathBuf};
use std::time::Duration;

use daemon_core::model_work::cause::{Cause, Containment};
use daemon_core::model_work::launcher;
use model_launcher::{list, profile};

const APPARMOR_ENABLED: &str = "/sys/module/apparmor/parameters/enabled";
const RESTRICT_USERNS: &str = "/proc/sys/kernel/apparmor_restrict_unprivileged_userns";
const PROFILES: &str = "/sys/kernel/security/apparmor/profiles";
/// The most bytes of a file of the system that this module reads.
const SYSTEM_FILE_BYTES: usize = 1 << 20;
/// The longest wait for `--protocol`.
const PROTOCOL_WAIT: Duration = Duration::from_secs(5);
/// The most bytes of the protocol line that are read.
const PROTOCOL_BYTES: u64 = 256;

/// The launcher of one start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Way {
    /// The daemon program in its launch mode.
    Daemon,
    /// The model launcher at [`model_launcher::PATH`], with the kept signed list.
    Root { list: PathBuf, signature: PathBuf },
}

/// Whether this system needs the model launcher.
#[must_use]
pub fn is_used() -> bool {
    launcher::is_used(
        read(APPARMOR_ENABLED).as_deref(),
        read(RESTRICT_USERNS).as_deref(),
    )
}

/// The launcher of a start of `program`, with the kept list in the model state folder `state`.
///
/// # Errors
/// `containment_failed` with `launcher_missing` or `launcher_outdated`; `runtime_not_signed`
/// when the program is not on the kept list, or no valid list is kept.
pub async fn way(program: &Path, state: &Path) -> Result<Way, Cause> {
    if !is_used() {
        return Ok(Way::Daemon);
    }
    tokio::task::spawn_blocking(check)
        .await
        .unwrap_or(Err(Containment::LauncherMissing))
        .map_err(Cause::ContainmentFailed)?;
    let list = state.join(list::FILE);
    let signature = state.join(list::SIGNATURE_FILE);
    let (program, kept, kept_signature) = (program.to_owned(), list.clone(), signature.clone());
    let signed =
        tokio::task::spawn_blocking(move || on_kept_list(&program, &kept, &kept_signature))
            .await
            .unwrap_or(false);
    if !signed {
        return Err(Cause::RuntimeNotSigned);
    }
    Ok(Way::Root { list, signature })
}

/// The installed launcher is in order: only root can change it and each folder above it, its
/// profile is loaded, and its protocol is the one of this daemon. It waits up to 5 seconds for
/// `--protocol`, so it runs on a blocking thread.
///
/// # Errors
/// `launcher_missing` or `launcher_outdated`.
pub fn check() -> Result<(), Containment> {
    use std::os::unix::fs::MetadataExt as _;
    let path = Path::new(model_launcher::PATH);
    for (index, part) in path.ancestors().enumerate() {
        let metadata = std::fs::symlink_metadata(part).map_err(|_| Containment::LauncherMissing)?;
        let kind = if index == 0 {
            metadata.file_type().is_file()
        } else {
            metadata.file_type().is_dir()
        };
        if !kind || !launcher::only_root_writes(metadata.uid(), metadata.mode()) {
            return Err(Containment::LauncherMissing);
        }
    }
    let profiles = read(PROFILES).ok_or(Containment::LauncherMissing)?;
    if !launcher::profile_loaded(&profiles, profile::NAME) {
        return Err(Containment::LauncherMissing);
    }
    protocol()
}

/// The protocol of the installed launcher is the one of this daemon.
fn protocol() -> Result<(), Containment> {
    use std::io::Read as _;
    let mut child = std::process::Command::new(model_launcher::PATH)
        .arg(model_launcher::PROTOCOL_ARGUMENT)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| Containment::LauncherMissing)?;
    let end = std::time::Instant::now() + PROTOCOL_WAIT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Containment::LauncherMissing);
            }
        }
    };
    let mut output = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = stdout.take(PROTOCOL_BYTES).read_to_end(&mut output);
    }
    if !status.success() {
        return Err(Containment::LauncherOutdated);
    }
    launcher::protocol(&output, &model_launcher::protocol_line())
}

/// Whether `program` is on the kept list, whose signature holds.
fn on_kept_list(program: &Path, list: &Path, signature: &Path) -> bool {
    let (Some(key), Some(program)) = (model_launcher::updater_key(), program.to_str()) else {
        return false;
    };
    let signed = model_launcher::launch::Signed {
        list,
        signature,
        key: &key,
    };
    model_launcher::launch::is_signed(program, &signed)
}

/// The text of a file of the system, when it reads.
fn read(path: &str) -> Option<String> {
    let bytes = super::read_capped(Path::new(path), SYSTEM_FILE_BYTES)?;
    String::from_utf8(bytes).ok()
}
