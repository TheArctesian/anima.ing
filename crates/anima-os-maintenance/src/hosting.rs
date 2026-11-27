//! Fence managed Docker dispatch against a disruptive node operation.
use std::{
    fs::{self, File, OpenOptions},
    io::Read as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Error, RECEIPT, ROOT, lock_named, write_receipt};

pub const SYSTEM_SOCKET: &str = "/run/docker.sock";
const LOCK: &str = "hosting.lock";
const DISPATCH: &str = "/run/anima-os/hosting-dispatch.json";

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Apply,
    Start,
    Restart,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    schema: u32,
    id: Uuid,
    operation: Operation,
}

/// A command with a recorded dispatch intent. Dropping it leaves uncertain intent visible.
pub struct Dispatch {
    _lock: File,
}

impl Dispatch {
    /// # Errors
    /// Maintenance or another dispatch has ownership, or the intent could not be synced.
    pub fn begin(operation: Operation) -> Result<Self, Error> {
        let guard = open_dispatch()?;
        write_receipt(
            DISPATCH,
            "hosting-dispatch.pending",
            &Receipt {
                schema: 1,
                id: Uuid::now_v7(),
                operation,
            },
        )?;
        Ok(Self { _lock: guard })
    }

    /// # Errors
    /// Stable completion could not be recorded.
    pub fn finished(self) -> Result<(), Error> {
        clear()
    }
}

/// # Errors
/// A managed Docker command would be refused now.
pub fn available() -> Result<(), Error> {
    open_dispatch().map(drop)
}

fn open_dispatch() -> Result<File, Error> {
    let guard = exclude()?;
    if Path::new(RECEIPT).try_exists()? {
        return Err(Error::Refused("The node is in maintenance."));
    }
    Ok(guard)
}

pub(super) fn exclude() -> Result<File, Error> {
    let guard = lock_named(LOCK)?;
    if status()?.is_some() {
        return Err(Error::Refused(
            "A Docker command needs reconciliation. Check the hosting dispatch before proceeding.",
        ));
    }
    Ok(guard)
}

/// # Errors
/// The hosting dispatch receipt is unavailable or invalid.
pub fn status() -> Result<Option<Receipt>, Error> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(DISPATCH)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err(Error::Refused(
            "The hosting dispatch receipt is not protected.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(513).read_to_end(&mut bytes)?;
    let receipt: Receipt = serde_json::from_slice(&bytes)?;
    if bytes.len() > 512 || receipt.schema != 1 || receipt.id.is_nil() {
        return Err(Error::Refused("The hosting dispatch receipt is invalid."));
    }
    Ok(Some(receipt))
}

/// Clear interrupted intent after the local owner verifies Docker has no pending command.
/// # Errors
/// The confirmation or receipt does not match, a command is still active, or storage failed.
pub fn release(id: &str, confirmation: &std::ffi::OsStr) -> Result<(), Error> {
    if confirmation != "--verified-no-pending-command" {
        return Err(Error::Refused(
            "Verify that the previous Docker command has ended first.",
        ));
    }
    let _guard = lock_named(LOCK)?;
    if status()?.is_none_or(|receipt| id != receipt.id.to_string()) {
        return Err(Error::Refused(
            "The hosting dispatch changed. Read its receipt again.",
        ));
    }
    clear()
}

fn clear() -> Result<(), Error> {
    fs::remove_file(DISPATCH)?;
    File::open(ROOT)?.sync_all()?;
    Ok(())
}
