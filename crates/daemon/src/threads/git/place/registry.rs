//! Active worktree records and the checked storage shared by their journal and receipts.

use std::io;
use std::path::Path;

use uuid::Uuid;

use super::records::Directory;
use super::{BadPlace, Home, Record};

impl Home {
    /// Writes the record of a worktree, in a folder and a file for the user alone.
    ///
    /// # Errors
    /// [`BadPlace`] for an invalid name or an inaccessible record.
    pub fn keep(&self, thread: Uuid, repository: &str, record: &Record) -> Result<(), BadPlace> {
        let file = self.record_file(thread, repository)?;
        let text = serde_json::to_vec_pretty(record).map_err(|_| BadPlace::NotOurWorktree)?;
        self.write_state(&file, &text)
            .map_err(|_| BadPlace::NotOurWorktree)
    }

    /// Removes the record of a worktree that was removed.
    pub fn forget(&self, thread: Uuid, repository: &str) {
        let _ = self.release(thread, repository);
    }

    pub(in super::super) fn release(&self, thread: Uuid, repository: &str) -> io::Result<()> {
        let file = self
            .record_file(thread, repository)
            .map_err(io::Error::other)?;
        self.remove_state(&file)
    }

    /// The record of the worktree as the runner wrote it.
    ///
    /// # Errors
    /// [`BadPlace`] for an invalid name, or if the record is missing or cannot be read.
    pub fn recorded(&self, thread: Uuid, repository: &str) -> Result<Record, BadPlace> {
        self.active(thread, repository)
            .map_err(|_| BadPlace::NotOurWorktree)?
            .ok_or(BadPlace::NotOurWorktree)
    }

    pub(in super::super) fn active(
        &self,
        thread: Uuid,
        repository: &str,
    ) -> io::Result<Option<Record>> {
        let file = self
            .record_file(thread, repository)
            .map_err(io::Error::other)?;
        self.read_state(&file)?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(io::Error::from))
            .transpose()
    }

    pub(in super::super) fn read_state(&self, file: &Path) -> io::Result<Option<Vec<u8>>> {
        match self.state_parent(file, false) {
            Ok(folder) => folder.read(file_name(file)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(in super::super) fn write_state(&self, file: &Path, bytes: &[u8]) -> io::Result<()> {
        self.state_parent(file, true)?
            .write(file_name(file)?, bytes)
    }

    pub(in super::super) fn remove_state(&self, file: &Path) -> io::Result<()> {
        match self.state_parent(file, false) {
            Ok(folder) => folder.remove(file_name(file)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(in super::super) fn private_git_exists(&self, path: &Path) -> io::Result<bool> {
        match Directory::open(self.records(), path, false) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub(in super::super) fn state_parent(
        &self,
        file: &Path,
        create: bool,
    ) -> io::Result<Directory> {
        let parent = file.parent().ok_or(io::ErrorKind::InvalidInput)?;
        Directory::open(self.records(), parent, create)
    }
}

fn file_name(path: &Path) -> io::Result<&std::ffi::OsStr> {
    path.file_name().ok_or(io::ErrorKind::InvalidInput.into())
}
