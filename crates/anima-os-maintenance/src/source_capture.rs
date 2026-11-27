//! Keep a source copy visible to maintenance after admission is released.
use std::fs::{File, TryLockError};

use super::{Admission, Error, open_lock};

const LOCK: &str = "source-captures.lock";

pub struct Activity {
    _lock: File,
}

impl Activity {
    /// Register a live capture while holding node admission.
    ///
    /// # Errors
    /// The activity lock is unavailable.
    pub fn begin(_admission: &Admission) -> Result<Self, Error> {
        let lock = open_lock(LOCK)?;
        lock.try_lock_shared().map_err(|error| match error {
            TryLockError::WouldBlock => Error::Busy,
            TryLockError::Error(error) => Error::Io(error),
        })?;
        Ok(Self { _lock: lock })
    }
}

pub(super) fn active() -> Result<bool, Error> {
    let lock = open_lock(LOCK)?;
    match lock.try_lock() {
        Ok(()) => Ok(false),
        Err(TryLockError::WouldBlock) => Ok(true),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}
