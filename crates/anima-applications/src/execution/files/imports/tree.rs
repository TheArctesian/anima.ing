//! Verify that a prepared bind tree is independent of the source and sync it without following links.
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    os::{
        fd::OwnedFd,
        unix::{ffi::OsStrExt as _, fs::MetadataExt as _},
    },
};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags};

use super::{Fault, binds};
use crate::execution::files::{RESOLVE, directory};

type Links = HashMap<(u64, u64), (u64, u64)>;

const MAX_ENTRIES: usize = 1_000_000;
const MAX_DEPTH: usize = 64;

pub(super) fn verify(folder: &OwnedFd) -> Result<Vec<String>, Fault> {
    let mut entries = 0;
    for entry in Dir::read_from(folder)? {
        match entry?.file_name().to_bytes() {
            b"." | b".." => (),
            b"binds" | b"migration.json" => entries += 1,
            _ => return Err(Fault::UnsafePlan),
        }
    }
    if entries != 2 {
        return Err(Fault::UnsafePlan);
    }
    let root = binds(folder)?;
    let mut names = Vec::new();
    let mut remaining = MAX_ENTRIES;
    let mut links = Links::new();
    for entry in Dir::read_from(&root)? {
        let entry = entry?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        let name = std::str::from_utf8(bytes).map_err(|_| Fault::UnsafePlan)?;
        if names.len() >= 64 || !compose_rules::forms::is_name(name) {
            return Err(Fault::UnsafePlan);
        }
        let volume = directory(&root, OsStr::new(name))?;
        sync_tree(&volume, 0, &mut remaining, &mut links)?;
        names.push(name.to_owned());
    }
    if names.is_empty() || links.values().any(|(seen, total)| seen != total) {
        return Err(Fault::UnsafePlan);
    }
    fs::fsync(&root)?;
    names.sort_unstable();
    Ok(names)
}

fn sync_tree(
    folder: &OwnedFd,
    depth: usize,
    remaining: &mut usize,
    links: &mut Links,
) -> Result<(), Fault> {
    if depth >= MAX_DEPTH {
        return Err(Fault::UnsafePlan);
    }
    for entry in Dir::read_from(folder)? {
        let entry = entry?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        *remaining = remaining.checked_sub(1).ok_or(Fault::UnsafePlan)?;
        let name = OsStr::from_bytes(bytes);
        let stat = fs::statat(folder, name, AtFlags::SYMLINK_NOFOLLOW)?;
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => {
                sync_tree(&directory(folder, name)?, depth + 1, remaining, links)?;
            }
            FileType::RegularFile => sync_file(folder, name, links)?,
            FileType::Symlink => (),
            _ => return Err(Fault::UnsafePlan),
        }
    }
    Ok(fs::fsync(folder)?)
}

fn sync_file(folder: &OwnedFd, name: &OsStr, links: &mut Links) -> Result<(), Fault> {
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOFOLLOW;
    let file = File::from(fs::openat2(folder, name, flags, Mode::empty(), RESOLVE)?);
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(Fault::UnsafePlan);
    }
    if metadata.nlink() > 1 {
        let (seen, total) = links
            .entry((metadata.dev(), metadata.ino()))
            .or_insert((0, metadata.nlink()));
        *seen += 1;
        if *total != metadata.nlink() {
            return Err(Fault::UnsafePlan);
        }
        if *seen > 1 {
            return Ok(());
        }
    }
    Ok(file.sync_all()?)
}
