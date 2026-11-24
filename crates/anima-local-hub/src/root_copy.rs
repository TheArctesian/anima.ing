//! A copy of the whole root of a stopped hub, with a dump of its database, and its check (ROL-054, R-189).
//!
//! ```text
//! <copy>/root/          each file of the hub root, but the programs, the locks and the sockets
//! <copy>/database.dump  `pg_dump` of the hub's database, made directly before the files
//! <copy>/copy.json      the list of the entries with their sizes, modes and SHA-256
//! ```
//!
//! The copy holds every secret of the hub: it keeps the owner-only modes of the root, and nothing of a file is printed.

mod database;
mod entries;
mod list;

#[cfg(all(test, unix))]
mod tests;

use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anima_database::backup::{self, BackupError};
use anima_database::{Tree, TreeError, owner_only};

use crate::run::{self, RunError};
use crate::{Carried, Folders};
pub use list::{Entry, Kind, List};

pub const DUMP: &str = "database.dump";
pub const LIST: &str = "copy.json";
pub const ROOT: &str = "root";

#[derive(Debug, thiserror::Error)]
pub enum RootCopyError {
    #[error("the hub of this folder runs: stop it, then make the copy")]
    HubRuns,
    #[error("this folder holds no hub: there is no database below it")]
    NoHub,
    #[error("the place of the copy is there already; give a path that does not exist")]
    PlaceTaken,
    #[error("the folder of the copy can be written by other users")]
    SharedFolder,
    #[error("cannot {step} ({kind})")]
    Io {
        step: &'static str,
        kind: io::ErrorKind,
    },
    #[error("the database did not start for the dump: {0}")]
    Database(String),
    #[error(transparent)]
    Dump(#[from] BackupError),
    #[error(transparent)]
    Tree(#[from] TreeError),
    #[error("the copy has no list that reads; it is not a copy of a hub root, or it is damaged")]
    NoList,
    #[error("the copy is damaged: {0}")]
    Damaged(String),
}

pub(crate) fn io_error(step: &'static str) -> impl FnOnce(io::Error) -> RootCopyError {
    move |error| RootCopyError::Io {
        step,
        kind: error.kind(),
    }
}

/// What a copy holds, for the line the command prints; no path and no secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Summary {
    pub entries: usize,
    pub bytes: u64,
    /// When the copy was made, in seconds of Unix time.
    pub made: u64,
}

/// Makes a copy of the root of `folders` in the new folder `to`; the hub must be stopped.
///
/// # Errors
/// [`RootCopyError::HubRuns`] for a hub that runs; otherwise the step that failed.
pub fn make(carried: &Carried, folders: &Folders, to: &Path) -> Result<Summary, RootCopyError> {
    if !folders.database().cluster().is_dir() {
        return Err(RootCopyError::NoHub);
    }
    let _lock = run::lock(folders).map_err(|error| match error {
        RunError::AnotherCopy => RootCopyError::HubRuns,
        RunError::Io { step, kind } => RootCopyError::Io { step, kind },
    })?;
    new_place(to)?;
    let made = fill(carried, folders, to);
    if made.is_err() {
        fs::remove_dir_all(to).ok();
    }
    made
}

fn fill(carried: &Carried, folders: &Folders, to: &Path) -> Result<Summary, RootCopyError> {
    database::dump(carried, folders, &to.join(DUMP))?;
    let mut list = entries::copy(folders, &to.join(ROOT))?;
    list.dump = Some(entries::file_entry(&to.join(DUMP), Path::new(DUMP))?);
    list.made = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    list.write(&to.join(LIST))?;
    Ok(list.summary())
}

fn new_place(to: &Path) -> Result<(), RootCopyError> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(RootCopyError::PlaceTaken);
    }
    let above = to
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    #[cfg(windows)]
    if !anima_windows::files::secure(&above, false).map_err(io_error("inspect the copy folder"))? {
        return Err(RootCopyError::SharedFolder);
    }
    #[cfg(unix)]
    let above = fs::metadata(above).map_err(io_error("look at the folder of the copy"))?;
    #[cfg(unix)]
    if above.permissions().mode() & 0o022 != 0 {
        return Err(RootCopyError::SharedFolder);
    }
    owner_only::folder(to).map_err(io_error("make the folder of the copy"))
}

/// Checks each entry of the copy's list and that the dump reads; it starts no server.
///
/// # Errors
/// [`RootCopyError::NoList`] or [`RootCopyError::Damaged`] with the first fault, by path only.
pub fn check(tree: &Path, copy: &Path) -> Result<Summary, RootCopyError> {
    let list = List::read(&copy.join(LIST))?;
    list.has_the_parts_of_a_hub()?;
    for entry in &list.entries {
        entries::verify(&copy.join(ROOT), entry)?;
    }
    let dump = list
        .dump
        .as_ref()
        .ok_or_else(|| RootCopyError::Damaged("the list names no dump".to_owned()))?;
    entries::verify(copy, dump)?;
    if !backup::reads(&Tree::at(tree)?, &copy.join(DUMP))? {
        return Err(RootCopyError::Damaged(
            "pg_restore cannot read the dump".to_owned(),
        ));
    }
    Ok(list.summary())
}
