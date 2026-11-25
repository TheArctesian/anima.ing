//! Private crash-durable launch and command records, serialized across holder processes.

use std::{
    collections::BTreeMap,
    fs::File,
    io,
    os::unix::fs::MetadataExt as _,
    path::{Path, PathBuf},
};

use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{command::SavedCommand, launch::Reservation};
use crate::config::{home::Home, private_file};

const JOURNAL: &str = "remote-code.json";
const LOCK: &str = "remote-code.lock";
const LIMIT: usize = 8 * 1024 * 1024;
const RECORDS: usize = 8192;
const ADMISSION_ROOM: usize = LIMIT - 2 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("remote Code storage is unavailable")]
    Io(#[from] io::Error),
    #[error("remote Code private storage is unavailable")]
    Private(#[from] private_file::PrivateFileError),
    #[error("remote Code storage is busy")]
    Busy,
    #[error("remote Code storage contains an invalid record")]
    Invalid,
    #[error("remote Code storage is full")]
    Full,
    #[error("the original remote Code binding differs")]
    Conflict,
}

/// Concurrent processes use the same lock, including across a journal rename.
pub struct Store(PathBuf);

#[derive(Serialize, Deserialize)]
pub(super) struct Journal {
    pub(super) key: [u8; 32],
    pub(super) launches: BTreeMap<Uuid, Reservation>,
    pub(super) commands: BTreeMap<Uuid, SavedCommand>,
}

pub(super) struct Locked<'a> {
    _lock: File,
    pub(super) journal: Journal,
    dir: &'a std::path::Path,
}

impl Store {
    #[must_use]
    pub fn at(home: &Home) -> Self {
        Self(home.dir().to_owned())
    }

    pub(super) fn lock(&self) -> Result<Locked<'_>, StoreError> {
        if !private_file::folder_private(&self.0)? {
            return Err(StoreError::Invalid);
        }
        let fd = open(
            self.0.join(LOCK),
            OFlags::CREATE | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(io::Error::from)?;
        let file = File::from(fd);
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || private_file::exposed(
                metadata.mode(),
                metadata.uid(),
                private_file::running_user()?,
            )
        {
            return Err(StoreError::Invalid);
        }
        flock(&file, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
            if error == rustix::io::Errno::WOULDBLOCK {
                StoreError::Busy
            } else {
                io::Error::from(error).into()
            }
        })?;
        let journal = Journal::load(&self.0.join(JOURNAL))?;
        Ok(Locked {
            _lock: file,
            journal,
            dir: &self.0,
        })
    }
}

impl Journal {
    fn load(path: &Path) -> Result<Self, StoreError> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Self::new(),
            Err(error) => return Err(error.into()),
        };
        let length = usize::try_from(metadata.len()).map_err(|_| StoreError::Invalid)?;
        if !metadata.is_file() || length > LIMIT {
            return Err(StoreError::Invalid);
        }
        // The exclusive lock prevents another writer from moving the record during the read.
        // Use its measured size so the private buffer allocates and wipes only that much.
        let bytes = private_file::read_within(path, length)?.ok_or(StoreError::Invalid)?;
        serde_json::from_slice(&bytes).map_err(|_| StoreError::Invalid)
    }

    fn new() -> Result<Self, StoreError> {
        let mut key = [0; 32];
        getrandom::fill(&mut key).map_err(|_| StoreError::Invalid)?;
        Ok(Self {
            key,
            launches: BTreeMap::new(),
            commands: BTreeMap::new(),
        })
    }

    pub(super) fn admit(&self) -> Result<(), StoreError> {
        if self.launches.len() + self.commands.len() >= RECORDS {
            return Err(StoreError::Full);
        }
        Ok(())
    }
}

impl Locked<'_> {
    pub(super) fn save_admission(&self) -> Result<(), StoreError> {
        self.write(ADMISSION_ROOM)
    }

    pub(super) fn save(&self) -> Result<(), StoreError> {
        self.write(LIMIT)
    }

    fn write(&self, limit: usize) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec(&self.journal).map_err(|_| StoreError::Invalid)?;
        if bytes.len() > limit {
            return Err(StoreError::Full);
        }
        Ok(private_file::write(self.dir, JOURNAL, &bytes)?)
    }
}
