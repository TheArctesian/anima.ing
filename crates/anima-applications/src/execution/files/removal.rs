//! Delete one entry or walk a tree without following links or mount points.
use std::{
    ffi::{OsStr, OsString},
    os::fd::OwnedFd,
    os::unix::ffi::OsStrExt as _,
};

use rustix::{
    fs::{self, AtFlags, Dir, FileType},
    io::Errno,
};

use super::{Fault, Target, directory};

pub(super) fn file(target: &Target<'_>) -> Result<(), Fault> {
    unlink(&target.dir, target.name, AtFlags::empty())?;
    Ok(fs::fsync(&target.dir)?)
}

pub(super) fn folder(target: &Target<'_>) -> Result<(), Fault> {
    let root = match directory(&target.dir, target.name) {
        Ok(root) => root,
        Err(Fault::Storage(std::io::ErrorKind::NotFound)) => return Ok(()),
        Err(error) => return Err(error),
    };
    empty(root, target.name, &target.dir)?;
    Ok(fs::fsync(&target.dir)?)
}

struct Tree {
    directory: OwnedFd,
    entries: Dir,
    name: OsString,
}

impl Tree {
    fn new(directory: OwnedFd, name: &OsStr) -> Result<Self, Fault> {
        let entries = Dir::read_from(&directory)?;
        Ok(Self {
            directory,
            entries,
            name: name.to_os_string(),
        })
    }
}

fn empty(root: OwnedFd, name: &OsStr, parent: &OwnedFd) -> Result<(), Fault> {
    let mut trees = vec![Tree::new(root, name)?];
    while let Some(tree) = trees.last_mut() {
        if let Some(entry) = tree.entries.next() {
            let entry = entry?;
            let bytes = entry.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            if let Some(folder) = descend(&tree.directory, OsStr::from_bytes(bytes))? {
                trees.push(folder);
            }
            continue;
        }
        let Some(tree) = trees.pop() else { break };
        let parent = trees.last().map_or(parent, |tree| &tree.directory);
        unlink(parent, &tree.name, AtFlags::REMOVEDIR)?;
    }
    Ok(())
}

fn descend(parent: &OwnedFd, name: &OsStr) -> Result<Option<Tree>, Fault> {
    let metadata = match fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) => metadata,
        Err(Errno::NOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if FileType::from_raw_mode(metadata.st_mode) == FileType::Directory {
        return Tree::new(directory(parent, name)?, name).map(Some);
    }
    unlink(parent, name, AtFlags::empty())?;
    Ok(None)
}

fn unlink(parent: &OwnedFd, name: &OsStr, flags: AtFlags) -> Result<(), Fault> {
    match fs::unlinkat(parent, name, flags) {
        Ok(()) | Err(Errno::NOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
