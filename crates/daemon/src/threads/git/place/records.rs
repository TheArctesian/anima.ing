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
    let real = |path: &Path| std::fs::canonicalize(path).ok();
    match (real(folder), real(records)) {
        (Some(folder), Some(records)) => {
            records.starts_with(&folder) || folder.starts_with(&records)
        }
        // Records that are not made yet are where their path says.
        (Some(folder), None) => records.starts_with(&folder) || folder.starts_with(records),
        (None, _) => true,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn above() -> (tempfile::TempDir, PathBuf) {
        let keep = tempfile::tempdir().expect("a folder");
        let real = std::fs::canonicalize(keep.path()).expect("its real path");
        (keep, real)
    }

    fn mode(path: &Path, mode: u32) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("a mode");
    }

    #[test]
    fn the_folder_is_made_for_its_owner_alone_and_found_again() {
        let (_keep, above) = above();
        let place = above.join("anima-501");
        assert_eq!(opened(&place), Ok(place.clone()));
        let about = std::fs::metadata(&place).expect("the folder");
        assert_eq!(about.permissions().mode() & 0o777, 0o700);
        assert_eq!(opened(&place), Ok(place));
        assert_eq!(
            standard(501).and_then(|place| place.file_name().map(ToOwned::to_owned)),
            Some("anima-501".into())
        );
    }

    #[test]
    fn a_folder_open_to_others_a_link_and_a_file_are_never_used() {
        let (_keep, above) = above();
        let open = above.join("open");
        std::fs::create_dir(&open).expect("a folder");
        mode(&open, 0o750);
        assert_eq!(opened(&open), Err(BadRecords::OpenToOthers));
        // What a different user can leave at the place first: a link to a folder of ours.
        let ours = above.join("ours");
        std::fs::create_dir(&ours).expect("a folder");
        mode(&ours, 0o700);
        let link = above.join("anima-link");
        std::os::unix::fs::symlink(&ours, &link).expect("a link");
        assert_eq!(opened(&link), Err(BadRecords::NotAFolder));
        assert_eq!(checked(&link), Err(BadRecords::NotAFolder));
        assert_eq!(checked(&above.join("nothing")), Err(BadRecords::Gone));
        let file = above.join("anima-file");
        std::fs::write(&file, "").expect("a file");
        assert_eq!(opened(&file), Err(BadRecords::NotAFolder));
        assert_eq!(opened(Path::new("relative")), Err(BadRecords::BadPath));
    }

    #[test]
    fn a_folder_above_that_others_can_change_is_refused_unless_entries_are_kept() {
        let (_keep, above) = above();
        let shared = above.join("shared");
        std::fs::create_dir(&shared).expect("a folder");
        mode(&shared, 0o777);
        assert_eq!(
            opened(&shared.join("anima-501")),
            Err(BadRecords::OpenAbove)
        );
        // As `/Users/Shared` and `/var/tmp`: each user writes, only the owner removes.
        mode(&shared, 0o1777);
        assert!(opened(&shared.join("anima-501")).is_ok());
        let through = above.join("through");
        std::os::unix::fs::symlink(&shared, &through).expect("a link");
        assert_eq!(
            opened(&through.join("anima-502")),
            Err(BadRecords::OpenAbove)
        );
    }

    #[test]
    fn a_touch_marks_each_file_of_the_records_as_used_now() {
        let (_keep, above) = above();
        let place = opened(&above.join("anima-501")).expect("the records");
        std::fs::create_dir_all(place.join("a/b")).expect("folders");
        let file = place.join("a/b/pack");
        std::fs::write(&file, "x").expect("a file");
        let long_ago = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_hours(24);
        let old = std::fs::FileTimes::new()
            .set_accessed(long_ago)
            .set_modified(long_ago);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .and_then(|file| file.set_times(old))
            .expect("old times");
        touch(&place).expect("a touch");
        let now = std::fs::metadata(&file)
            .and_then(|about| about.modified())
            .expect("a time");
        assert!(now > long_ago + std::time::Duration::from_hours(24));
    }

    #[test]
    fn a_thread_in_a_folder_above_the_records_holds_them() {
        let (_keep, above) = above();
        let records = above.join("shared/anima-501");
        std::fs::create_dir_all(&records).expect("a folder");
        std::fs::create_dir_all(above.join("home/project")).expect("a folder");
        assert!(held_by(&records, &above));
        assert!(held_by(&records, &above.join("shared")));
        assert!(held_by(&records, &records));
        std::fs::create_dir_all(records.join("inside")).expect("a folder");
        assert!(held_by(&records, &records.join("inside")));
        assert!(!held_by(&records, &above.join("home")));
        assert!(!held_by(&records, &above.join("home/project")));
        assert!(held_by(&records, &above.join("gone")));
        std::os::unix::fs::symlink(above.join("shared"), above.join("home/link")).expect("a link");
        assert!(held_by(&records, &above.join("home/link")));
    }
}
