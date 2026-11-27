//! Restore only into a fresh, private project directory, using directory handles.
use crate::linux::workspace::handoff::project::archive::{child, local_link};
use crate::linux::{Error, Result};
use nix::{
    fcntl::{AtFlags, OFlag},
    sys::stat::{Mode, fchmod, mkdirat},
    unistd::{Gid, Uid, fchown, fchownat, linkat, symlinkat},
};
use std::{
    ffi::OsStr,
    fs::{File, FileTimes},
    io::{self, Read},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};
use tar::EntryType;

const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const CREATE: OFlag = OFlag::O_WRONLY
    .union(OFlag::O_CREAT)
    .union(OFlag::O_EXCL)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const LOOKUP: OFlag = OFlag::O_PATH
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const OWNER: Option<Uid> = Some(Uid::from_raw(1000));
const GROUP: Option<Gid> = Some(Gid::from_raw(1000));

#[derive(Clone, Copy)]
pub(super) struct Attributes {
    pub mode: u32,
    pub mtime: u64,
}

pub(super) struct Node {
    pub kind: EntryType,
    pub path: PathBuf,
    pub link: Option<PathBuf>,
    pub attributes: Attributes,
    pub bytes: u64,
}

struct Directory {
    path: PathBuf,
    file: File,
    attributes: Attributes,
}

pub(super) struct Tree {
    root: File,
    directories: Vec<Directory>,
    quota: u64,
    bytes: u64,
}

impl Tree {
    pub(super) fn new(root: File, quota: u64) -> Self {
        Self {
            root,
            directories: Vec::new(),
            quota,
            bytes: 0,
        }
    }

    pub(super) fn root(&mut self, attributes: Attributes) -> Result<()> {
        self.directories.push(Directory {
            path: PathBuf::new(),
            file: self.root.try_clone()?,
            attributes,
        });
        Ok(())
    }

    pub(super) fn add(&mut self, node: &Node, contents: impl Read) -> Result<()> {
        let (parent, name) = location(&node.path)?;
        self.close_to(parent)?;
        let directory = self.directories.last().ok_or(invalid())?;
        match node.kind {
            EntryType::Directory => self.mkdir(name, node.path.clone(), node.attributes),
            EntryType::Regular => self.file(name, node, contents),
            EntryType::Symlink => link(&directory.file, name, node),
            EntryType::Link => hardlink(&self.root, &directory.file, name, node),
            _ => Err(invalid()),
        }
    }

    fn mkdir(&mut self, name: &OsStr, path: PathBuf, attributes: Attributes) -> Result<()> {
        let parent = &self.directories.last().ok_or(invalid())?.file;
        mkdirat(parent, name, Mode::S_IRWXU).map_err(io::Error::from)?;
        let file = child(parent, name, DIRECTORY)?;
        self.directories.push(Directory {
            path,
            file,
            attributes,
        });
        Ok(())
    }

    fn file(&mut self, name: &OsStr, node: &Node, mut contents: impl Read) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(node.bytes)
            .filter(|bytes| *bytes <= self.quota)
            .ok_or(Error::Refused(
                "The project exceeds the destination workspace quota.",
            ))?;
        let parent = &self.directories.last().ok_or(invalid())?.file;
        let mut file = child(parent, name, CREATE)?;
        if io::copy(&mut contents, &mut file)? != node.bytes {
            return Err(invalid());
        }
        finish(&file, node.attributes)
    }

    fn close_to(&mut self, parent: &Path) -> Result<()> {
        while self
            .directories
            .last()
            .is_some_and(|dir| dir.path != parent)
        {
            let entry = self.directories.pop().ok_or(invalid())?;
            finish(&entry.file, entry.attributes)?;
        }
        if self.directories.is_empty() {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<()> {
        while let Some(directory) = self.directories.pop() {
            finish(&directory.file, directory.attributes)?;
        }
        Ok(())
    }
}

fn finish(file: &File, attributes: Attributes) -> Result<()> {
    fchown(file, OWNER, GROUP).map_err(io::Error::from)?;
    fchmod(file, Mode::from_bits_truncate(attributes.mode)).map_err(io::Error::from)?;
    let time = UNIX_EPOCH
        .checked_add(Duration::from_secs(attributes.mtime))
        .ok_or(invalid())?;
    file.set_times(FileTimes::new().set_modified(time))?;
    file.sync_all()?;
    Ok(())
}

fn link(parent: &File, name: &OsStr, node: &Node) -> Result<()> {
    let target = node
        .link
        .as_ref()
        .filter(|target| local_link(&node.path, target))
        .ok_or(invalid())?;
    symlinkat(target, parent, name).map_err(io::Error::from)?;
    fchownat(parent, name, OWNER, GROUP, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
    Ok(())
}

fn hardlink(root: &File, parent: &File, name: &OsStr, node: &Node) -> Result<()> {
    let target = node.link.as_ref().ok_or(invalid())?;
    let (source, leaf) = location(target)?;
    let source = if source.as_os_str().is_empty() {
        root.try_clone()?
    } else {
        child(root, source.as_os_str(), DIRECTORY)?
    };
    if !child(&source, leaf, LOOKUP)?.metadata()?.is_file() {
        return Err(invalid());
    }
    linkat(&source, leaf, parent, name, AtFlags::empty()).map_err(io::Error::from)?;
    Ok(())
}

fn location(path: &Path) -> Result<(&Path, &OsStr)> {
    let name = path.as_os_str().as_bytes();
    if name.len() >= 4096
        || name
            .split(|byte| *byte == b'/')
            .any(|piece| matches!(piece, b"" | b"." | b"..") || piece.contains(&0))
        || path.components().count() > 64
    {
        return Err(invalid());
    }
    Ok((
        path.parent().ok_or(invalid())?,
        path.file_name().ok_or(invalid())?,
    ))
}

pub(super) fn invalid() -> Error {
    Error::Refused("The checkpoint archive contains an invalid entry or path.")
}
