//! Atomically moves the original directory between open parents, without replacing a path.

use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{Mode, OFlags, RenameFlags, fstat, mkdirat, open, openat, renameat_with};

use super::{FolderId, RETAINED, Record};

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(super) fn retire(record: &Record, container: &Path, original: FolderId) -> io::Result<()> {
    let home = open(&record.home, DIRECTORY, Mode::empty())?;
    let retired = private(&home, RETAINED.as_ref())?;
    let id = container.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let destination = private(&retired, id)?;
    let source = record.folder.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let parent = open(source, DIRECTORY, Mode::empty())?;
    let name = record
        .folder
        .file_name()
        .ok_or(io::ErrorKind::InvalidInput)?;
    if FolderId::of(&record.folder) != Some(original) {
        return Err(io::Error::other("the original worktree directory changed"));
    }
    renameat_with(
        &parent,
        name,
        &destination,
        "worktree",
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        if error == rustix::io::Errno::XDEV {
            io::Error::other("the recovery folder is on another filesystem")
        } else {
            error.into()
        }
    })?;
    if FolderId::of(&container.join("worktree")) != Some(original) {
        renameat_with(
            &destination,
            "worktree",
            &parent,
            name,
            RenameFlags::NOREPLACE,
        )?;
        return Err(io::Error::other("the original worktree directory changed"));
    }
    Ok(())
}

fn private(parent: &OwnedFd, name: &std::ffi::OsStr) -> io::Result<OwnedFd> {
    match mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(error.into()),
    }
    let dir = openat(parent, name, DIRECTORY, Mode::empty())?;
    let stat = fstat(&dir)?;
    let owner = rustix::process::getuid().as_raw();
    if stat.st_uid != owner || stat.st_mode & 0o077 != 0 {
        return Err(io::Error::other(
            "the recovery folder is not private to this user",
        ));
    }
    Ok(dir)
}
