use super::{Entry, Outcome};
use crate::linux::{Error, Result, command};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use uuid::Uuid;

const ENTRIES: usize = 50;
const BYTES: u64 = 65_536;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: u8,
    entries: Vec<Entry>,
}

pub(super) struct History<'a> {
    directory: &'a Path,
    owner: u32,
}

impl<'a> History<'a> {
    pub(super) const fn new(directory: &'a Path, owner: u32) -> Self {
        Self { directory, owner }
    }

    pub(super) fn read(&self) -> Result<Vec<Entry>> {
        self.protected_directory()?;
        let path = self.directory.join("actions.json");
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        self.protected_file(&file)?;
        let mut bytes = Vec::new();
        file.take(BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > BYTES {
            return Err(Error::Refused("Action history exceeded its size limit."));
        }
        let ledger: Ledger = serde_json::from_slice(&bytes)?;
        if ledger.schema != 1 || ledger.entries.len() > ENTRIES {
            return Err(Error::Refused("Action history uses an unreadable format."));
        }
        Ok(ledger.entries)
    }

    pub(super) fn append(&self, entry: Entry) -> Result<()> {
        let _guard = self.lock()?;
        let mut entries = self.read()?;
        entries.insert(0, entry);
        entries.truncate(ENTRIES);
        self.save(entries)
    }

    pub(super) fn finish(&self, id: Uuid, outcome: Outcome) -> Result<()> {
        let _guard = self.lock()?;
        let mut entries = self.read()?;
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            entry.outcome = outcome;
            self.save(entries)?;
        }
        Ok(())
    }

    fn save(&self, entries: Vec<Entry>) -> Result<()> {
        let ledger = Ledger { schema: 1, entries };
        let bytes = serde_json::to_vec(&ledger)?;
        if bytes.len() as u64 > BYTES {
            return Err(Error::Refused("Action history exceeded its size limit."));
        }
        command::atomic(&self.directory.join("actions.json"), &bytes, 0o600)
    }

    fn protected_directory(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(self.directory)?;
        if !metadata.is_dir() || metadata.uid() != self.owner || metadata.mode() & 0o077 != 0 {
            return Err(Error::Refused("Action history directory is not protected."));
        }
        Ok(())
    }

    fn protected_file(&self, file: &File) -> Result<()> {
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != self.owner || metadata.mode() & 0o077 != 0 {
            return Err(Error::Refused("Action history file is not protected."));
        }
        Ok(())
    }

    fn lock(&self) -> Result<File> {
        self.protected_directory()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(self.directory.join("actions.lock"))?;
        self.protected_file(&lock)?;
        lock.lock()?;
        Ok(lock)
    }
}

#[cfg(test)]
mod tests;
