//! The place of the records and of the runner's Git folders (ruling of the operator): outside
//! the home folder of the person, so that a thread that starts in the home folder, which is
//! allowed with a warning (THR-041), cannot write them. The Git stage is safe only while no
//! thread in a sandbox can write this place (THR-050). A thread in Full access has no sandbox
//! and can write it; by the operator's ruling of 2026-11-23 that is an accepted cost.
//!
//! The place is in a folder that each user of the computer can write (`/Users/Shared`,
//! `/var/tmp`). A different user can make a folder or a link of the same name first. So the
//! folder is made new with access for its owner alone. Each access checks that it is a
//! real folder, of this user, with no access for others, in a folder where only its owner can
//! remove or rename it. A place that fails a check is never used.
//!
//! No backup tool and no special access of the system is needed: `/Users/Shared` is a normal
//! folder for each program of a user, and Time Machine keeps it.
//!
//! I/O: makes one folder and reads the metadata of it and of the folders above it.

use std::path::{Path, PathBuf};

use agent_threads::git::GitRefusal;

#[cfg(unix)]
mod store;
#[cfg(not(unix))]
#[path = "records/unsupported.rs"]
mod store;
pub(crate) use store::Directory;

/// Why the place of the records is not used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BadRecords {
    #[error("this system has no place for the records of Anima's Git outside the home folder")]
    NoPlace,
    #[error("the path of the records is relative, has `..` or has no folder above it")]
    BadPath,
    #[error("a folder above the records is a link, or a different user can change what it holds")]
    OpenAbove,
    #[error("the place of the records is a link or a file, not a folder")]
    NotAFolder,
    #[error("the folder of the records was made by a different user of this computer")]
    NotOurs,
    #[error("the folder of the records gives access to other users")]
    OpenToOthers,
    #[error("the folder of the records could not be made or read")]
    Unreadable,
    #[error("the folder of the records is not there any more; start Anima again to make it")]
    Gone,
    #[error("the records of Anima's Git are in a temporary folder that an agent can write")]
    InTemp,
}

/// The place of the records for the user with the number `user`: one folder for each user,
/// so two accounts of one computer never share it.
#[must_use]
pub fn standard(user: u32) -> Option<PathBuf> {
    let above = if cfg!(target_os = "macos") {
        "/Users/Shared"
    } else if cfg!(unix) {
        "/var/tmp"
    } else {
        return None;
    };
    Some(Path::new(above).join(format!("anima-{user}")))
}

/// The number of the user that this program runs as.
#[cfg(unix)]
#[must_use]
pub fn this_user() -> u32 {
    rustix::process::getuid().as_raw()
}

impl From<BadRecords> for std::io::Error {
    fn from(bad: BadRecords) -> Self {
        let kind = match bad {
            BadRecords::Gone => std::io::ErrorKind::NotFound,
            BadRecords::OpenAbove | BadRecords::NotOurs | BadRecords::OpenToOthers => {
                std::io::ErrorKind::PermissionDenied
            }
            _ => std::io::ErrorKind::Other,
        };
        Self::new(kind, bad)
    }
}

/// Makes and validates the owner-only records folder and its ancestry.
///
/// # Errors
/// [`BadRecords`] if the directory cannot be safely made or opened.
#[cfg(unix)]
pub fn opened(place: &Path) -> Result<PathBuf, BadRecords> {
    store::root_folder(place, true)?;
    Ok(place.to_owned())
}

#[cfg(not(unix))]
pub fn opened(_place: &Path) -> Result<PathBuf, BadRecords> {
    Err(BadRecords::NoPlace)
}

/// Checks the current records folder and all its ancestors without following links.
///
/// # Errors
/// [`BadRecords`] if the folder is absent or a permission or identity check fails.
#[cfg(unix)]
pub fn checked(place: &Path) -> Result<(), BadRecords> {
    store::root_folder(place, false).map(drop)
}

#[cfg(not(unix))]
pub const fn checked(_place: &Path) -> Result<(), BadRecords> {
    Err(BadRecords::NoPlace)
}

/// Marks the records as used so that the system does not expire them.
///
/// # Errors
/// The filesystem error if a checked directory cannot be accessed or marked.
pub fn touch(place: &Path) -> std::io::Result<()> {
    let now = std::time::SystemTime::now();
    let times = std::fs::FileTimes::new()
        .set_accessed(now)
        .set_modified(now);
    Directory::open(place, place, false)?.touch(times)
}

/// Whether a Git action may run now: not on Windows (ruling of the operator: "not available on
/// Windows yet"), not while a thread holds the records (`held`), and only while the records
/// are a folder of this user alone.
///
/// # Errors
/// The refusal that says why.
pub fn open_for_actions(records: &Path, held: bool) -> Result<(), GitRefusal> {
    if cfg!(windows) {
        return Err(GitRefusal::NotOnWindows);
    }
    if held {
        return Err(GitRefusal::RecordsHeld);
    }
    checked(records).map_err(|bad| GitRefusal::Failed {
        said: bad.to_string(),
    })
}

/// A thread whose folder is `folder` holds the records at `records`: its agent can write
/// them, because the folder is above them or inside them. A folder that cannot be resolved
/// counts as one that holds them.
#[must_use]
pub fn held_by(records: &Path, folder: &Path) -> bool {
    match (std::fs::canonicalize(folder), resolved(records)) {
        (Ok(folder), Some(records)) => records.starts_with(&folder) || folder.starts_with(&records),
        _ => true,
    }
}

/// The real path of `path`. For a path that is not made yet, such as records before the first
/// start: the real path of the nearest folder above it that is there, with the rest of `path`
/// after it, so that a link above it (`/tmp` on a Mac) is followed.
fn resolved(path: &Path) -> Option<PathBuf> {
    path.ancestors().find_map(|above| {
        let real = std::fs::canonicalize(above).ok()?;
        let rest = path.strip_prefix(above).ok()?;
        // A missing part then `..` would point somewhere else than the path says.
        super::plain(rest).then(|| real.join(rest))
    })
}

/// A running thread in a sandbox holds the records at `records` when one of `roots`, the
/// folders that its sandbox lets its agent write, holds them ([`held_by`]): its folder, its
/// `TMPDIR`, `/tmp`, and each more root of the sandbox (THR-050). An agent in Full access has
/// no sandbox and can write each folder of the person: this check does not cover it (operator's
/// ruling of 2026-11-23).
#[must_use]
pub fn held_by_any(records: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| held_by(records, root))
}

#[cfg(all(test, unix))]
mod tests;
