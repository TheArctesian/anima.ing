//! No link is followed and no path or text crosses in an error (LOG-007).

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::{
    fs::{DirBuilder, Metadata},
    os::unix::fs::{DirBuilderExt, MetadataExt},
};

use super::mach_o;
use crate::builder::{artifact, error::BuilderError};

#[cfg(unix)]
fn private(kind: &Metadata) -> bool {
    kind.is_dir() && kind.uid() == nix::unistd::geteuid().as_raw() && kind.mode() & 0o022 == 0
}

/// Makes `folder` anew, mode 0700, so the recipe trusts nothing an earlier run left (D-1).
#[cfg(unix)]
pub fn renew(folder: &Path) -> Result<(), BuilderError> {
    let parent = folder.parent().ok_or(BuilderError::Bundle)?;
    let real = fs::canonicalize(parent).map_err(|_| BuilderError::Bundle)?;
    let kind = fs::symlink_metadata(parent).map_err(|_| BuilderError::Bundle)?;
    if real != parent || !private(&kind) {
        return Err(BuilderError::Bundle);
    }
    match fs::symlink_metadata(folder) {
        Ok(kind) if kind.is_dir() => fs::remove_dir_all(folder),
        Ok(_) => fs::remove_file(folder),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
    .map_err(|_| BuilderError::Bundle)?;
    DirBuilder::new()
        .mode(0o700)
        .create(folder)
        .map_err(|_| BuilderError::Bundle)?;
    let made = fs::symlink_metadata(folder).map_err(|_| BuilderError::Bundle)?;
    (private(&made) && made.mode() & 0o777 == 0o700)
        .then_some(())
        .ok_or(BuilderError::Bundle)
}

/// A file git ignores would change the build and never show in `git status`.
pub fn absent(path: &Path) -> Result<(), BuilderError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err(BuilderError::Dirty),
    }
}

/// No control character, so no line `codesign` prints with the path can look like another line.
fn plain(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| !name.chars().any(char::is_control))
}

/// Every regular file below `app` whose first bytes are a Mach-O magic (D-4); links are not followed.
pub fn mach_o_files(clone: &Path, app: &Path) -> Result<Vec<PathBuf>, BuilderError> {
    let mut folders = vec![artifact::own_folder(clone, app)?];
    let mut found = Vec::new();
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).map_err(|_| BuilderError::Bundle)? {
            let entry = entry.map_err(|_| BuilderError::Bundle)?;
            if !plain(&entry.file_name()) {
                return Err(BuilderError::Bundle);
            }
            let kind = entry.file_type().map_err(|_| BuilderError::Bundle)?;
            if kind.is_dir() {
                folders.push(entry.path());
            } else if kind.is_file() && mach_o::is_mach_o(&first_bytes(clone, &entry.path())?) {
                found.push(entry.path());
            }
        }
    }
    found.sort();
    Ok(found)
}

fn first_bytes(clone: &Path, path: &Path) -> Result<Vec<u8>, BuilderError> {
    let mut bytes = Vec::new();
    artifact::open_built(clone, path)?
        .take(4)
        .read_to_end(&mut bytes)
        .map_err(|_| BuilderError::Bundle)?;
    Ok(bytes)
}

#[cfg(windows)]
pub fn renew(folder: &Path) -> Result<(), BuilderError> {
    let parent = folder.parent().ok_or(BuilderError::Bundle)?;
    anima_windows::files::directory(parent).map_err(|_| BuilderError::Bundle)?;
    match fs::symlink_metadata(folder) {
        Ok(_) => {
            anima_windows::files::directory(folder).map_err(|_| BuilderError::Bundle)?;
            fs::remove_dir_all(folder).map_err(|_| BuilderError::Bundle)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(BuilderError::Bundle),
    }
    anima_windows::files::directory(folder).map_err(|_| BuilderError::Bundle)
}
