//! The mark of an operator's stop: an update leaves a hub with this mark stopped.

use std::fs;
use std::io;

use anima_database::owner_only;

use crate::Folders;

pub(crate) fn set(folders: &Folders) -> io::Result<()> {
    owner_only::folder(folders.root())?;
    owner_only::new_file(&folders.stopped_by_operator()).map(drop)
}

pub(crate) fn clear(folders: &Folders) -> io::Result<()> {
    match fs::remove_file(folders.stopped_by_operator()) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

pub(crate) fn is_set(folders: &Folders) -> bool {
    folders.stopped_by_operator().symlink_metadata().is_ok()
}
