use super::super::{Error, Result};
use super::worker::Strategy;
use anima_os_maintenance::capacity::Resources;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const ROOT: &str = "/persist/anima-builds";
pub const MAX_SECONDS: u64 = 3900;
pub const fn default_seconds() -> std::num::NonZeroU64 {
    std::num::NonZeroU64::MIN.saturating_add(MAX_SECONDS - 1)
}

pub const QUOTA: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervisor: Option<Uuid>,
    pub schema: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<super::lease::Lease>,
    pub id: Uuid,
    pub context: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub resources: Resources,
    #[serde(default = "default_seconds")]
    pub seconds: std::num::NonZeroU64,
    pub phase: String,
    pub source_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<super::super::workspace::checkout::Source>,
    pub strategy: Option<Strategy>,
    #[serde(default)]
    pub requested_strategy: Option<Strategy>,
    pub result: Option<Value>,
    #[serde(default)]
    pub toolchain: Option<Value>,
}

pub fn base(id: Uuid) -> PathBuf {
    Path::new(ROOT).join(id.to_string())
}
pub fn unit(id: Uuid) -> String {
    format!("anima-build-{id}.service")
}

pub fn lock(id: Uuid) -> Result<fs::File> {
    lock_named(id, &id.to_string())
}

pub(super) fn lock_verification(id: Uuid) -> Result<fs::File> {
    lock_named(id, &format!("{id}.verify"))
}

fn lock_named(id: Uuid, name: &str) -> Result<fs::File> {
    if id.is_nil() {
        return Err(Error::Refused("Choose a valid build attempt identity."));
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(Path::new(ROOT).join("locks").join(name))?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => Error::BuildAttemptBusy,
        fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(file)
}

pub fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Refused("Build metadata must be a regular file."));
    }
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err(Error::Refused("Build state exceeded its limit."));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn required(id: Uuid) -> Result<Attempt> {
    let attempt: Attempt = read(&base(id).join("attempt.json"))?;
    if attempt.schema != 1 || attempt.id != id {
        return Err(Error::Refused("Build attempt state is invalid."));
    }
    Ok(attempt)
}

pub fn save(attempt: &Attempt) -> Result<()> {
    let pending = base(attempt.id).join("attempt.pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(attempt)?)?;
    file.sync_all()?;
    fs::rename(pending, base(attempt.id).join("attempt.json"))?;
    fs::File::open(base(attempt.id))?.sync_all()?;
    Ok(())
}

pub fn reserved_bytes() -> Result<u64> {
    let mut total = 0_u64;
    if !Path::new(ROOT).try_exists()? {
        return Ok(total);
    }
    for entry in fs::read_dir(ROOT)? {
        let name = entry?.file_name();
        if name == "locks" {
            continue;
        }
        let id = name
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
            .ok_or(Error::Refused("Build storage inventory is invalid."))?;
        let attempt = required(id)?;
        // Verification follows worker termination and a read-only seal. The
        // retained archive and scratch already occupy space in the filesystem.
        let sealed = attempt.phase == "verified"
            && attempt
                .result
                .as_ref()
                .is_some_and(|result| result["integrityVerified"] == true);
        if attempt.phase != "deleted" && !sealed {
            total = total
                .checked_add(QUOTA)
                .ok_or(Error::Refused("Build storage accounting overflowed."))?;
        }
    }
    Ok(total)
}

pub fn require_released(workspace: Uuid) -> Result<()> {
    if !Path::new(ROOT).try_exists()? {
        return Ok(());
    }
    for (count, entry) in fs::read_dir(ROOT)?.enumerate() {
        if count >= 16384 {
            return Err(Error::Refused("Build inventory exceeded its bound."));
        }
        let name = entry?.file_name();
        if name == "locks" {
            continue;
        }
        let id = name
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
            .ok_or(Error::Refused("Build inventory is invalid."))?;
        let attempt = required(id)?;
        if attempt.workspace == workspace && attempt.phase != "deleted" {
            return Err(Error::Refused(
                "Release this workspace's build attempts before deleting it.",
            ));
        }
    }
    Ok(())
}
