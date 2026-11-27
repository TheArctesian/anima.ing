//! One admission/maintenance boundary shared by the session and updater.
#![cfg(target_os = "linux")]
mod admission;
pub mod capacity;
pub mod cutover;
pub mod drain;
pub mod hosting;
mod observation;
pub mod pressure;
mod reconcile;
pub mod recovery;
pub mod reservations;
pub mod source_capture;
pub mod source_pipeline;
pub mod update_window;
pub use admission::{WorkAdmission, work_admission};
pub use observation::impact;
pub use reconcile::{release, status};

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::SystemTime,
};
use uuid::Uuid;

const ROOT: &str = "/run/anima-os";
const RECEIPT: &str = "/run/anima-os/maintenance.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("Another maintenance or admission operation is running.")]
    Busy,
    #[error("Hosting is under memory pressure. Queue the work or choose another node.")]
    HostingPressure,
    #[error("No recent hosting pressure observation. Queue the work or choose another node.")]
    PressureUnavailable,
    #[error("Node operation state could not be accessed.")]
    Io(#[from] std::io::Error),
    #[error("Maintenance state is invalid; reconcile it before retrying.")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Restart,
    Shutdown,
    Activate,
    Rollback,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    schema: u32,
    id: Uuid,
    operation: Operation,
    phase: String,
    admission_closed: bool,
}

/// Held across observation and dispatch. A dispatched/uncertain action keeps
/// admission closed until reboot or explicit operator reconciliation.
pub struct Lease {
    _lock: File,
    _hosting: File,
    receipt: Receipt,
    retain: bool,
}

/// Acquire before a workspace writer lock. Keep until acceptance is persisted;
/// after that the reservation, not the caller's connection, owns the capacity.
pub struct Admission {
    _lock: File,
    pipeline: Option<source_pipeline::Pipeline>,
    opened_at: SystemTime,
}

impl Admission {
    /// # Errors
    /// Refuses active maintenance or another admission transaction.
    pub fn acquire() -> Result<Self, Error> {
        let guard = Self::configuration()?;
        recovery::require_ready()?;
        Ok(guard)
    }

    /// Carry the exact source admission across its workspace and build stages.
    ///
    /// # Errors
    /// Recovery, maintenance or the source admission record refuses continuation.
    pub fn for_pipeline(pipeline: Option<source_pipeline::Pipeline>) -> Result<Self, Error> {
        let mut guard = Self::acquire()?;
        if let Some(pipeline) = pipeline {
            pipeline.require_current()?;
        }
        guard.pipeline = pipeline;
        Ok(guard)
    }

    /// Serialize changes to boot configuration and durable Hosting authority.
    ///
    /// # Errors
    /// A maintenance action is in progress or the lock cannot be acquired.
    pub fn configuration() -> Result<Self, Error> {
        let guard = Self::cleanup()?;
        if Path::new(RECEIPT).try_exists()? {
            return Err(Error::Refused(
                "The node is in maintenance. Queue work or choose another node.",
            ));
        }
        Ok(guard)
    }

    /// Recheck the operator's drain and current hosting pressure before taking new work.
    ///
    /// # Errors
    /// Active drain, hosting pressure, stale observation or inaccessible admission state.
    pub fn require_open_work(&self) -> Result<(), Error> {
        match self.pipeline {
            Some(pipeline) => pipeline.require_current()?,
            None => drain::require_open(self.opened_at)?,
        }
        pressure::require_clear()
    }

    /// Cleanup may drain work while admission is closed, but cannot admit work.
    /// # Errors
    /// Refuses another transaction or inaccessible state.
    pub fn cleanup() -> Result<Self, Error> {
        Ok(Self {
            _lock: lock()?,
            pipeline: None,
            opened_at: SystemTime::now(),
        })
    }
}

fn lock() -> Result<File, Error> {
    lock_named("maintenance.lock")
}

fn require_protected_state() -> Result<(), Error> {
    let directory = fs::symlink_metadata(ROOT)?;
    if !directory.is_dir() || directory.uid() != 0 || directory.mode() & 0o077 != 0 {
        return Err(Error::Refused("Maintenance directory is not protected."));
    }
    Ok(())
}

fn lock_named(name: &str) -> Result<File, Error> {
    let file = open_lock(name)?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => Error::Busy,
        fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(file)
}

fn open_lock(name: &str) -> Result<File, Error> {
    require_protected_state()?;
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(Path::new(ROOT).join(name))?)
}

fn save(receipt: &Receipt) -> Result<(), Error> {
    write_receipt(RECEIPT, "maintenance.pending", receipt)
}

fn read_small_receipt<T: serde::de::DeserializeOwned>(path: &str) -> Result<Option<T>, Error> {
    read_receipt(path, 256)
}

fn read_receipt<T: serde::de::DeserializeOwned>(
    path: &str,
    limit: u64,
) -> Result<Option<T>, Error> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err(Error::Refused(
            "The node operation record is not protected.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err(Error::Refused(
            "The node operation record exceeded its limit.",
        ));
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn write_receipt(path: &str, temporary: &str, receipt: &impl Serialize) -> Result<(), Error> {
    let temporary = Path::new(ROOT).join(temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(receipt)?)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    File::open(ROOT)?.sync_all()?;
    Ok(())
}

impl Lease {
    /// Closes admission before observing workloads. An unavailable engine is
    /// unknown, never an empty inventory.
    ///
    /// # Errors
    /// Refuses concurrent, unresolved, active or unobservable work.
    pub async fn prepare(operation: Operation) -> Result<Self, Error> {
        let lock = lock()?;
        if Path::new(RECEIPT).try_exists()? {
            return Err(Error::Refused(
                "A previous maintenance request needs reconciliation before retrying.",
            ));
        }
        let hosting = hosting::exclude()?;
        let lease = Self {
            _lock: lock,
            _hosting: hosting,
            retain: false,
            receipt: Receipt {
                schema: 1,
                id: Uuid::now_v7(),
                operation,
                phase: "preparing".into(),
                admission_closed: true,
            },
        };
        save(&lease.receipt)?;
        observation::quiescent().await?;
        Ok(lease)
    }

    /// Record intent before the first externally visible maintenance effect.
    /// # Errors
    /// Refuses dispatch when the receipt cannot be persisted.
    pub fn dispatching(&mut self) -> Result<(), Error> {
        self.receipt.phase = "dispatching".into();
        save(&self.receipt)?;
        self.retain = true;
        Ok(())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if !self.retain
            && let Err(error) = fs::remove_file(RECEIPT)
        {
            eprintln!("anima-os-maintenance: admission remains closed: {error}");
        }
    }
}
