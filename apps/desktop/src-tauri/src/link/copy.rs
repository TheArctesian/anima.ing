//! The copy of the link program that a unit of the user runs (HMD-058).

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::LinkError;
use crate::daemon::{placement, this_user};

/// The app makes `link` with mode 0700, so the link's check of the program's folder passes on any system (DEF-007).
pub fn destination(application: &Path) -> PathBuf {
    application.join("link/program").join(super::PROGRAM)
}

/// The place before DEF-007: a unit made then runs it until the app moves the unit to [`destination`].
pub fn earlier_copy(home: &Path) -> PathBuf {
    home.join(".local/bin").join(super::PROGRAM)
}

pub fn current_copy(application: &Path, home: &Path) -> PathBuf {
    let copy = destination(application);
    let earlier = earlier_copy(home);
    if !copy.is_file() && earlier.is_file() {
        earlier
    } else {
        copy
    }
}

pub fn same(one: &Path, other: &Path) -> bool {
    match (fs::read(one), fs::read(other)) {
        (Ok(one), Ok(other)) => one == other,
        _ => false,
    }
}

pub fn place(carried: &Path, application: &Path) -> Result<PathBuf, LinkError> {
    let destination = destination(application);
    if same(carried, &destination) {
        return Ok(destination);
    }
    let moved_aside = keep_running_copy(&destination);
    placement::place(carried, &destination, this_user()).map_err(|_| {
        // The copy of before goes back, so the unit runs on.
        if moved_aside && fs::symlink_metadata(&destination).is_err() {
            let _ = fs::rename(previous(&destination), &destination);
        }
        LinkError::Folder
    })
}

pub fn previous(copy: &Path) -> PathBuf {
    let mut name = copy.as_os_str().to_owned();
    name.push(".previous");
    PathBuf::from(name)
}

/// A rename, not an overwrite: the daemon checks the code signature, and macOS rejects a running program whose file was written over (HMD-054).
/// An earlier [`previous`] stays, as the running link may run from it.
pub(super) fn keep_running_copy(destination: &Path) -> bool {
    let aside = previous(destination);
    destination.is_file()
        && fs::symlink_metadata(&aside).is_err()
        && fs::rename(destination, aside).is_ok()
}

pub fn unplace(program: &Path) {
    let _ = fs::remove_file(program);
}

pub fn waits_for_restart(program: &Path) -> bool {
    fs::symlink_metadata(previous(program)).is_ok()
}

pub fn forget_previous(program: &Path) {
    unplace(&previous(program));
}
