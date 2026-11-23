//! The folders of model work (Section 10): made with mode 0700, and refused when a different
//! user owns one or a link replaces it.

use std::fs::DirBuilder;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
use std::path::{Component, Path};

use rustix::fs::{Mode, OFlags};

use daemon_core::model_work::folders::Folders;

/// Checks each folder that it makes, and `path`; a parent that was already there is used as it is.
///
/// # Errors
/// An I/O error, or `PermissionDenied` when a folder is a link, not a folder, or owned by a
/// different user.
pub fn private_folder(path: &Path) -> io::Result<()> {
    let missing: Vec<&Path> = path
        .ancestors()
        .take_while(|part| {
            !part.as_os_str().is_empty()
                && matches!(std::fs::symlink_metadata(part), Err(error) if error.kind() == io::ErrorKind::NotFound)
        })
        .collect();
    for part in missing.into_iter().rev() {
        match DirBuilder::new().mode(0o700).create(part) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        own_plain_folder(part)?;
    }
    own_plain_folder(path)
}

fn own_plain_folder(path: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    let own = meta.uid() == rustix::process::geteuid().as_raw();
    if meta.file_type().is_symlink() || !meta.is_dir() || !own {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "a folder of model work is not a folder of this user",
        ));
    }
    Ok(())
}

/// Makes each part relative to an open handle of its parent with `O_NOFOLLOW`, mode 0700 and owned
/// by this user, and refuses a link at any part. Gives the handle of the last folder (Section 10).
///
/// # Errors
/// An I/O error; `PermissionDenied` for a part that a different user owns; `InvalidInput` for
/// a part that is not a plain name.
pub fn private_below(root: &Path, relative: &Path) -> io::Result<OwnedFd> {
    private_folder(root)?;
    let flags = OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::RDONLY | OFlags::CLOEXEC;
    let mut dir = rustix::fs::open(root, flags, Mode::empty())?;
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a part of a folder of model work is not a name",
            ));
        };
        let made = match rustix::fs::mkdirat(&dir, name, Mode::from_raw_mode(0o700)) {
            Ok(()) => true,
            Err(rustix::io::Errno::EXIST) => false,
            Err(error) => return Err(error.into()),
        };
        let next = rustix::fs::openat(&dir, name, flags, Mode::empty())?;
        if made {
            rustix::fs::fchmod(&next, Mode::from_raw_mode(0o700))?;
        }
        if rustix::fs::fstat(&next)?.st_uid != rustix::process::geteuid().as_raw() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a folder of model work is not a folder of this user",
            ));
        }
        dir = next;
    }
    Ok(dir)
}

/// # Errors
/// An I/O error of `statvfs`.
pub fn free_bytes(path: &Path) -> io::Result<u64> {
    let stat = rustix::fs::statvfs(path)?;
    Ok(stat.f_bavail.saturating_mul(stat.f_frsize))
}

/// # Errors
/// As [`private_folder`].
pub fn make_all(folders: &Folders) -> io::Result<()> {
    [&folders.runtimes, &folders.models, &folders.state]
        .into_iter()
        .try_for_each(|path| private_folder(path))
}

/// No link is followed.
#[must_use]
pub fn bytes_below(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => bytes_below(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |meta| meta.len()),
            _ => 0,
        })
        .sum()
}
