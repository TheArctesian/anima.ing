use std::io::{Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use daemon_core::inventory::mobile::{self, Android, Apple, MobileTools, Probe, State};
use serde::de::DeserializeOwned;

use crate::readers::command::{Bounds, CommandError, capture};

pub(super) const MODE: &str = "__mobile-inventory";
const DEADLINE: Duration = Duration::from_secs(5);

pub(super) async fn collect() -> MobileTools {
    let mut found = MobileTools {
        apple: Apple {
            simulator_runtimes: read("runtimes").await,
            device_types: read("types").await,
        },
        android: Android {
            sdk_packages: read("sdk").await,
            emulator: read("emulator").await,
        },
    };
    if serde_json::to_vec(&found).is_ok_and(|bytes| bytes.len() > mobile::WIRE_LIMIT) {
        found = MobileTools::unavailable(State::TooLarge);
    }
    found
}

async fn read<T: DeserializeOwned>(kind: &'static str) -> Probe<T> {
    let Ok(program) = std::env::current_exe() else {
        return Probe::Unavailable(State::Exited);
    };
    let mut command = tokio::process::Command::new(program);
    command
        .arg(MODE)
        .arg(kind)
        .env_clear()
        .env("LC_ALL", "C")
        .current_dir("/");
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    if kind == "sdk" {
        // Only the explicitly configured SDK root; no personal preferences or account files.
        if let Some(root) =
            std::env::var_os("ANDROID_SDK_ROOT").or_else(|| std::env::var_os("ANDROID_HOME"))
        {
            command.arg(root);
        }
    }
    collect_probe(&mut command).await
}

async fn collect_probe<T: DeserializeOwned>(command: &mut tokio::process::Command) -> Probe<T> {
    let bounds = Bounds {
        limit: DEADLINE,
        keep_cut_output: false,
        output_limit: (mobile::WIRE_LIMIT + 1) as u64,
    };
    // Dropping capture also kills its group; the outer deadline bounds the reap path.
    let captured =
        tokio::time::timeout(DEADLINE, capture(command, "mobile inventory", bounds)).await;
    match captured.unwrap_or(Err(CommandError::Timeout {
        program: "mobile inventory",
    })) {
        Ok(bytes) if bytes.len() > mobile::WIRE_LIMIT => Probe::Unavailable(State::TooLarge),
        Ok(bytes) => {
            serde_json::from_slice(&bytes).unwrap_or(Probe::Unavailable(State::InvalidOutput))
        }
        Err(CommandError::Timeout { .. }) => Probe::Unavailable(State::TimedOut),
        Err(CommandError::Start { .. } | CommandError::Failed { .. }) => {
            Probe::Unavailable(State::Exited)
        }
    }
}

/// The parent owns the process group and deadline, including all synchronous filesystem work.
pub(super) fn run() -> ! {
    let kind = std::env::args().nth(2).unwrap_or_default();
    let encoded = match kind.as_str() {
        "runtimes" => serde_json::to_vec(&apple(
            mobile::SIMULATOR_RUNTIMES,
            mobile::simulator_runtimes,
        )),
        "types" => serde_json::to_vec(&apple(mobile::DEVICE_TYPES, mobile::device_types)),
        "emulator" => serde_json::to_vec(&from_output(
            output(mobile::EMULATOR.name, mobile::EMULATOR.args),
            mobile::emulator,
        )),
        "sdk" => {
            let root = std::env::args_os().nth(3).map(PathBuf::from);
            let found = root.map_or(Probe::Unavailable(State::Missing), |root| {
                super::sdk::read(&root)
            });
            serde_json::to_vec(&found)
        }
        _ => std::process::exit(2),
    };
    let result = encoded
        .ok()
        .and_then(|bytes| std::io::stdout().write_all(&bytes).ok());
    std::process::exit(i32::from(result.is_none()))
}

fn apple<T>(
    program: daemon_core::inventory::Program,
    parse: impl FnOnce(&[u8]) -> Probe<T>,
) -> Probe<T> {
    if !cfg!(target_os = "macos") {
        return Probe::Unavailable(State::Unsupported);
    }
    let selected = match output("xcode-select", &["-p"]) {
        Ok(selected) => selected,
        Err(state) => return Probe::Unavailable(state),
    };
    let root = std::path::Path::new(std::ffi::OsStr::from_bytes(selected.trim_ascii_end()));
    if !root.is_absolute() || !root.is_dir() {
        return Probe::Unavailable(State::Missing);
    }
    from_output(output(program.name, program.args), parse)
}

fn from_output<T>(
    output: Result<Vec<u8>, State>,
    parse: impl FnOnce(&[u8]) -> Probe<T>,
) -> Probe<T> {
    output.map_or_else(Probe::Unavailable, |bytes| parse(&bytes))
}

fn output(program: &str, args: &[&str]) -> Result<Vec<u8>, State> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                State::Missing
            } else {
                State::Exited
            }
        })?;
    let stdout = child.stdout.take().ok_or(State::Exited)?;
    let mut bytes = Vec::new();
    stdout
        .take((mobile::INPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| State::Exited)?;
    if bytes.len() > mobile::INPUT_LIMIT {
        return Err(State::TooLarge);
    }
    if !child.wait().map_err(|_| State::Exited)?.success() {
        return Err(State::Exited);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
