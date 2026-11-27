//! Resolve and check every entry on privileged startup paths.
use std::{
    collections::VecDeque,
    ffi::OsString,
    fs::{self, Metadata},
    os::unix::fs::{FileTypeExt as _, MetadataExt as _},
    path::{Component, Path, PathBuf},
};

use rustix::{
    fs::{Mode, StatVfsMountFlags, statvfs},
    io::Errno,
};

use super::Error;
use crate::start::Entry;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Settings,
    Program,
    Socket,
    PrivateDirectory,
    ConfigurationDirectory,
    ConfigurationFile,
}

pub(super) struct Resolved {
    pub real: PathBuf,
    pub metadata: Metadata,
    pub entries: Vec<Entry>,
}

pub(super) fn resolve(path: &Path, owner: u32, kind: Kind) -> Result<Resolved, Error> {
    if !path.is_absolute()
        || path.parent().is_none()
        || path.components().any(|part| part == Component::ParentDir)
    {
        return Err(Error::UntrustedPath);
    }
    let mut entries = vec![entry(&directory(Path::new("/"), owner)?)];
    let mut current = PathBuf::from("/");
    let mut parts = components(path);
    let mut links = 0;
    while let Some(part) = parts.pop_front() {
        match part {
            Part::Root => current = PathBuf::from("/"),
            Part::Up => {
                current.pop();
            }
            Part::Name(name) => {
                current.push(name);
                let metadata = fs::symlink_metadata(&current)?;
                if metadata.file_type().is_symlink() {
                    if (kind == Kind::Settings && parts.is_empty())
                        || !is_owner(&metadata, owner)
                        || links >= 40
                    {
                        return Err(Error::UntrustedPath);
                    }
                    let target = fs::read_link(&current)?;
                    follow(&mut parts, &mut current, &target);
                    links += 1;
                } else if parts.is_empty() {
                    return finish(current, metadata, entries, (owner, kind));
                } else {
                    entries.push(ancestor(&current, &metadata, owner, kind)?);
                }
            }
        }
    }
    Err(Error::UntrustedPath)
}

#[derive(Debug)]
enum Part {
    Root,
    Up,
    Name(OsString),
}

fn components(path: &Path) -> VecDeque<Part> {
    path.components()
        .filter_map(|component| match component {
            Component::RootDir => Some(Part::Root),
            Component::ParentDir => Some(Part::Up),
            Component::Normal(name) => Some(Part::Name(name.to_os_string())),
            _ => None,
        })
        .collect()
}

fn follow(parts: &mut VecDeque<Part>, current: &mut PathBuf, target: &Path) {
    current.pop();
    let mut next = components(target);
    next.append(parts);
    *parts = next;
}

fn finish(
    real: PathBuf,
    metadata: Metadata,
    mut entries: Vec<Entry>,
    (owner, kind): (u32, Kind),
) -> Result<Resolved, Error> {
    let valid = match kind {
        Kind::Settings => metadata.is_file() && metadata.uid() == 0,
        Kind::ConfigurationDirectory => metadata.is_dir() && metadata.uid() == owner,
        Kind::ConfigurationFile => metadata.is_file() && metadata.uid() == owner,
        Kind::Program => metadata.is_file() && metadata.mode() & 0o111 != 0,
        Kind::Socket => metadata.file_type().is_socket(),
        Kind::PrivateDirectory => {
            metadata.is_dir()
                && metadata.uid() == owner
                && !Mode::from_bits_truncate(metadata.mode()).intersects(Mode::RWXG | Mode::RWXO)
        }
    };
    if !valid {
        return Err(Error::UntrustedPath);
    }
    if kind != Kind::Socket {
        protected(&real, &metadata, owner)?;
        entries.push(entry(&metadata));
    }
    Ok(Resolved {
        real,
        metadata,
        entries,
    })
}

fn directory(path: &Path, owner: u32) -> Result<Metadata, Error> {
    let metadata = fs::symlink_metadata(path)?;
    check_directory(path, &metadata, owner)?;
    Ok(metadata)
}

fn check_directory(path: &Path, metadata: &Metadata, owner: u32) -> Result<(), Error> {
    if !metadata.is_dir() {
        return Err(Error::UntrustedPath);
    }
    protected(path, metadata, owner)
}

fn ancestor(path: &Path, metadata: &Metadata, owner: u32, kind: Kind) -> Result<Entry, Error> {
    let mut observed = entry(metadata);
    // NixOS gives its store root mode 1775 before it binds the store read-only.
    if kind == Kind::Program
        && path == Path::new("/nix/store")
        && metadata.is_dir()
        && metadata.uid() == 0
        && metadata.mode() & 0o7777 == 0o1775
        && statvfs(path)?.f_flag.contains(StatVfsMountFlags::RDONLY)
        && !has_acl(path)?
    {
        observed.mode &= !0o020;
    } else {
        check_directory(path, metadata, owner)?;
    }
    Ok(observed)
}

fn protected(path: &Path, metadata: &Metadata, owner: u32) -> Result<(), Error> {
    if !is_owner(metadata, owner) || metadata.mode() & 0o022 != 0 || has_acl(path)? {
        return Err(Error::UntrustedPath);
    }
    Ok(())
}

fn is_owner(metadata: &Metadata, owner: u32) -> bool {
    matches!(metadata.uid(), 0) || metadata.uid() == owner
}

pub(super) fn has_acl(path: &Path) -> Result<bool, Error> {
    match rustix::fs::lgetxattr_query(path, "system.posix_acl_access") {
        Ok(_) => Ok(true),
        Err(Errno::NODATA | Errno::NOTSUP) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn entry(metadata: &Metadata) -> Entry {
    Entry {
        owner: metadata.uid(),
        mode: metadata.mode(),
        group_members: Vec::new(),
        listed_writers: Vec::new(),
        listed_for_all: false,
    }
}

pub(super) fn text(path: PathBuf) -> Result<String, Error> {
    let text = path
        .into_os_string()
        .into_string()
        .map_err(|_| Error::UntrustedPath)?;
    crate::forms::is_path(&text)
        .then_some(text)
        .ok_or(Error::UntrustedPath)
}
