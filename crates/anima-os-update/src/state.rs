use crate::{Error, Result};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};

pub const DIRECTORY: &str = "/persist/anima-updates";
const STATE: &str = anima_os_maintenance::update_window::UPDATE_STATE;
const PROGRESS: &str = "/persist/anima-updates/progress.json";

pub fn prepare() -> Result<()> {
    fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(DIRECTORY)?;
    let directory = fs::symlink_metadata(DIRECTORY)?;
    if !directory.is_dir() || directory.uid() != 0 {
        return Err(Error::Refused("The update state directory is not trusted."));
    }
    fs::set_permissions(DIRECTORY, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Refused("The update state is not a regular file."));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::Refused(
            "The update metadata exceeds its size limit.",
        ));
    }
    Ok(bytes)
}

pub fn read() -> Result<Value> {
    match read_bounded(Path::new(STATE), 65536) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes)?;
            if !value.is_object()
                || !matches!(
                    value["phase"].as_str(),
                    Some("idle" | "current" | "staged" | "booting" | "rolled-back")
                )
            {
                return Err(Error::Refused(
                    "The retained update state is invalid; inspect it before retrying.",
                ));
            }
            if matches!(value["phase"].as_str(), Some("staged" | "booting"))
                && (!value["system"]
                    .as_str()
                    .is_some_and(crate::feed::valid_system)
                    || !value["previous"]
                        .as_str()
                        .is_some_and(crate::feed::valid_system))
            {
                return Err(Error::Refused("The retained update systems are invalid."));
            }
            Ok(value)
        }
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(json!({"phase":"idle"}))
        }
        Err(error) => Err(error),
    }
}

pub fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(
        path.parent()
            .ok_or(Error::Refused("The update path has no parent."))?,
    )?
    .sync_all()?;
    Ok(())
}

pub fn save(value: &Value) -> Result<()> {
    atomic(Path::new(STATE), &serde_json::to_vec(value)?)
}

fn review_boot() -> Result<Vec<u8>> {
    read_bounded(Path::new("/proc/sys/kernel/random/boot_id"), 64)
}

fn review_revision(action: &str, value: &Value, boot: &[u8]) -> Result<String> {
    let selection = ["phase", "version", "system", "previous"].map(|field| &value[field]);
    let digest = Sha256::digest(serde_json::to_vec(&(boot, action, selection))?);
    let mut revision = String::with_capacity(64);
    for byte in digest {
        let _ = write!(revision, "{byte:02x}");
    }
    Ok(revision)
}

pub fn require_review(action: &str, revision: &str) -> Result<()> {
    if revision != review_revision(action, &read()?, &review_boot()?)? {
        return Err(Error::ReviewChanged);
    }
    Ok(())
}

pub fn activation_revision() -> Result<String> {
    review_revision("activate", &read()?, &review_boot()?)
}

pub fn reviewed_status() -> Result<Value> {
    let mut value = status()?;
    let boot = review_boot()?;
    let activate = review_revision("activate", &value, &boot)?;
    value["activationWindow"] =
        crate::window::status(&activate).unwrap_or_else(|_| json!({"phase":"unavailable"}));
    value["review"] = json!({
        "activate": activate,
        "rollback": review_revision("rollback", &value, &boot)?,
    });
    Ok(value)
}

pub fn lock() -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/run/anima-update.lock")?)
}

pub fn writer() -> Result<File> {
    let lock = lock()?;
    lock.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => {
            Error::Refused("An update is already running. Wait for it to finish.")
        }
        std::fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(lock)
}

pub fn progress(phase: &str, received: Option<u64>, total: Option<u64>) -> Result<()> {
    atomic(
        Path::new(PROGRESS),
        &serde_json::to_vec(&json!({"phase":phase,"bytes":received,"totalBytes":total}))?,
    )
}

pub fn clear_progress() -> Result<()> {
    match fs::remove_file(PROGRESS) {
        Ok(()) => File::open(DIRECTORY)?.sync_all().map_err(Error::Io),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn status() -> Result<Value> {
    let mut value = read()?;
    let bytes = match read_bounded(Path::new(PROGRESS), 65536) {
        Ok(bytes) => bytes,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return read(),
        Err(error) => return Err(error),
    };
    let mut progress: Value = serde_json::from_slice(&bytes)?;
    if !progress.is_object() {
        return Err(Error::Refused("The update progress record is invalid."));
    }
    let lock = lock()?;
    match lock.try_lock() {
        Ok(()) => {
            if !Path::new(PROGRESS).exists() {
                return read();
            }
            progress["phase"] = json!("interrupted");
            value = read()?;
        }
        Err(std::fs::TryLockError::WouldBlock) => {}
        Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
    }
    value["operation"] = progress;
    Ok(value)
}
