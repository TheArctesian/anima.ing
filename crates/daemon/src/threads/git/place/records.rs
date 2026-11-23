//! The place of the records and of the runner's Git folders (ruling of the operator): outside
//! the home folder of the person, so that a thread that starts in the home folder, which is
//! allowed with a warning (THR-041), cannot write them. The Git stage is safe only while no
//! thread can write this place (THR-050).
//!
//! The place is in a folder that each user of the computer can write (`/Users/Shared`,
//! `/var/tmp`). A different user can make a folder or a link of the same name first. So the
//! folder is made new with access for its owner alone, and each start checks that it is a
//! real folder, of this user, with no access for others, in a folder where only its owner can
//! remove or rename it. A place that fails a check is never used.
//!
//! No backup tool and no special access of the system is needed: `/Users/Shared` is a normal
//! folder for each program of a user, and Time Machine keeps it.
//!
//! I/O: makes one folder and reads the metadata of it and of the folders above it.

use std::path::{Path, PathBuf};

use agent_threads::git::GitRefusal;

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

/// Makes the folder of the records at `place` when it is not there, and checks it and each
/// folder above it. The answer is `place`, safe to use until the next start.
///
/// # Errors
/// A [`BadRecords`] for each check that fails; nothing is used then.
#[cfg(unix)]
pub fn opened(place: &Path) -> Result<PathBuf, BadRecords> {
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, PermissionsExt as _};

    if !place.is_absolute() || !super::plain(place) {
        return Err(BadRecords::BadPath);
    }
    let us = this_user();
    for above in place.ancestors().skip(1) {
        let about = std::fs::symlink_metadata(above).map_err(|_| BadRecords::OpenAbove)?;
        // Only the system or this user owns it; where others can write, only the owner of an
        // entry can remove or rename that entry (the sticky bit).
        let owned = about.uid() == 0 || about.uid() == us;
        let closed = about.mode() & 0o022 == 0 || about.mode() & 0o1000 != 0;
        if !about.is_dir() || !owned || !closed {
            return Err(BadRecords::OpenAbove);
        }
    }
    let made = std::fs::DirBuilder::new().mode(0o700).create(place);
    match made {
        // The mode of a new folder does not depend on the mask of the process.
        Ok(()) => std::fs::set_permissions(place, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| BadRecords::Unreadable)?,
        Err(failed) if failed.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(BadRecords::Unreadable),
    }
    checked(place)?;
    Ok(place.to_owned())
}

/// Checks the folder of the records itself: a real folder of this user with no access for
/// others. Only [`opened`] makes it: a place with nothing at it is refused, so that nobody
/// else can make the folder between a check and a use.
///
/// # Errors
/// A [`BadRecords`] for nothing, a link, a file, a folder of a different user or one open to
/// others.
#[cfg(unix)]
pub fn checked(place: &Path) -> Result<(), BadRecords> {
    use std::os::unix::fs::MetadataExt as _;

    let about = match std::fs::symlink_metadata(place) {
        Ok(about) => about,
        Err(failed) if failed.kind() == std::io::ErrorKind::NotFound => {
            return Err(BadRecords::Gone);
        }
        Err(_) => return Err(BadRecords::Unreadable),
    };
    if !about.is_dir() {
        return Err(BadRecords::NotAFolder);
    }
    if about.uid() != this_user() {
        return Err(BadRecords::NotOurs);
    }
    if about.mode() & 0o077 != 0 {
        return Err(BadRecords::OpenToOthers);
    }
    Ok(())
}

/// Marks each file and folder of the records as used now. A system that removes what was not
/// used for a long time (`/var/tmp` on Linux, after about 30 days) then keeps them. The runner
/// calls it once a day while it runs.
///
/// # Errors
/// The error of the file system for the folder of the records itself; a file below it that
/// cannot be marked is left.
pub fn touch(place: &Path) -> std::io::Result<()> {
    let now = std::fs::FileTimes::new()
        .set_accessed(std::time::SystemTime::now())
        .set_modified(std::time::SystemTime::now());
    let mut folders = vec![place.to_owned()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder)?.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                folders.push(entry.path());
            } else if kind.is_file() {
                // A file that is read-only or gone in between is left: the next day comes.
                let _ = std::fs::File::open(entry.path()).and_then(|file| file.set_times(now));
            }
        }
        let _ = std::fs::File::open(&folder).and_then(|folder| folder.set_times(now));
    }
    Ok(())
}

/// A system with no owners and modes of this kind has no checked place yet.
#[cfg(not(unix))]
pub fn opened(_place: &Path) -> Result<PathBuf, BadRecords> {
    Err(BadRecords::NoPlace)
}

#[cfg(not(unix))]
pub const fn checked(_place: &Path) -> Result<(), BadRecords> {
    Ok(())
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
        Some(real.join(rest))
    })
}

/// A running thread holds the records at `records` when one of `roots`, the folders that its
/// agent can write, holds them ([`held_by`]): its folder, its `TMPDIR`, `/tmp`, and each more
/// folder that its sandbox lets it write (THR-050).
#[must_use]
pub fn held_by_any(records: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| held_by(records, root))
}

#[cfg(all(test, unix))]
mod tests;
