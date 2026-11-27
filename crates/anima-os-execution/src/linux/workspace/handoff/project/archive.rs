//! Deterministic, descriptor-relative traversal of the stopped workspace.
use super::{MAX_ENTRIES, digest::Output};
use crate::linux::{Error, Result};
use nix::{
    dir::Dir,
    fcntl::{OFlag, OpenHow, ResolveFlag, open, openat2, readlinkat},
    sys::stat::Mode,
};
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    io::{self, Read, Write},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Component, Path, PathBuf},
    time::Instant,
};
use tar::{Builder, EntryType, Header};

const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const NODE: OFlag = OFlag::O_PATH
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const FILE: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_CLOEXEC);

#[derive(Clone, Copy)]
pub(in crate::linux::workspace::handoff) struct Limits {
    pub quota: u64,
    pub bytes: u64,
    pub deadline: Instant,
}

pub(in crate::linux::workspace::handoff) fn open_project(home: &Path) -> Result<File> {
    let home = File::from(open(home, DIRECTORY, Mode::empty()).map_err(io::Error::from)?);
    child(&home, "workspace".as_ref(), DIRECTORY)
}

pub(in crate::linux::workspace::handoff) fn child(
    parent: &File,
    name: &std::ffi::OsStr,
    flags: OFlag,
) -> Result<File> {
    let how = OpenHow::new().flags(flags).resolve(
        ResolveFlag::RESOLVE_BENEATH
            | ResolveFlag::RESOLVE_NO_XDEV
            | ResolveFlag::RESOLVE_NO_SYMLINKS,
    );
    Ok(File::from(
        openat2(parent, name, how).map_err(io::Error::from)?,
    ))
}

pub(in crate::linux::workspace::handoff) fn write(
    directory: &File,
    output: impl Write,
    limits: Limits,
) -> Result<(String, u64, usize)> {
    let root = directory.metadata()?;
    let mut tree = Archive {
        tar: Builder::new(Output::new(output, limits.bytes, limits.deadline)),
        filesystem: root.dev(),
        quota: limits.quota,
        discovered: 0,
        bytes: 0,
        links: BTreeMap::new(),
    };
    tree.tar.append_data(
        &mut header(&root, EntryType::Directory, 0),
        ".",
        io::empty(),
    )?;
    tree.visit(directory, Path::new(""), &root)?;
    let count = tree.discovered;
    let (digest, bytes) = tree.tar.into_inner()?.finish()?;
    Ok((digest, bytes, count))
}

struct Archive<W: Write> {
    tar: Builder<Output<W>>,
    filesystem: u64,
    quota: u64,
    discovered: usize,
    bytes: u64,
    links: BTreeMap<(u64, u64), (PathBuf, Metadata)>,
}

impl<W: Write> Archive<W> {
    fn visit(&mut self, directory: &File, prefix: &Path, before: &Metadata) -> Result<()> {
        if prefix.components().count() >= 64 {
            return Err(Error::Refused(
                "Project directory nesting exceeded its limit.",
            ));
        }
        let mut dir =
            Dir::openat(directory, ".", DIRECTORY, Mode::empty()).map_err(io::Error::from)?;
        let mut names = Vec::new();
        for entry in dir.iter() {
            let entry = entry.map_err(io::Error::from)?;
            let name = entry.file_name().to_bytes();
            if matches!(name, b"." | b"..") {
                continue;
            }
            if self.discovered == MAX_ENTRIES {
                return Err(Error::Refused(
                    "The project checkpoint contains too many files.",
                ));
            }
            self.discovered += 1;
            names.push(std::ffi::OsStr::from_bytes(name).to_owned());
        }
        names.sort_unstable();
        for name in names {
            self.entry(directory, &name, &prefix.join(&name))?;
        }
        unchanged(directory, before)
    }

    fn entry(&mut self, parent: &File, name: &std::ffi::OsStr, path: &Path) -> Result<()> {
        if path.as_os_str().as_bytes().len() >= 4096 {
            return Err(Error::Refused(
                "A project path exceeds the checkpoint limit.",
            ));
        }
        let node = child(parent, name, NODE)?;
        let info = node.metadata()?;
        if info.dev() != self.filesystem {
            return Err(Error::Refused(
                "The project crosses into another filesystem.",
            ));
        }
        if info.is_dir() {
            self.tar.append_data(
                &mut header(&info, EntryType::Directory, 0),
                path,
                io::empty(),
            )?;
            self.visit(&node, path, &info)
        } else if info.is_symlink() {
            let target = PathBuf::from(readlinkat(&node, "").map_err(io::Error::from)?);
            if !local_link(path, &target) {
                return Err(Error::Refused(
                    "A project link cannot be preserved safely on the destination.",
                ));
            }
            symlink_entry(&mut self.tar, &info, path, target.as_os_str().as_bytes())?;
            unchanged(&node, &info)
        } else if info.is_file() {
            let file = child(parent, name, FILE)?;
            self.file(&file, path, &info)
        } else {
            Err(Error::Refused(
                "The project contains a special file that cannot be moved.",
            ))
        }
    }

    fn file(&mut self, file: &File, path: &Path, info: &Metadata) -> Result<()> {
        unchanged(file, info)?;
        let identity = (info.dev(), info.ino());
        if let Some((target, first)) = self.links.get(&identity) {
            unchanged(file, first)?;
            self.tar
                .append_link(&mut header(info, EntryType::Link, 0), path, target)?;
            return Ok(());
        }
        self.bytes = self
            .bytes
            .checked_add(info.len())
            .filter(|bytes| *bytes <= self.quota)
            .ok_or(Error::Refused(
                "Project contents exceed the source workspace quota.",
            ))?;
        let mut contents = file.take(info.len());
        self.tar.append_data(
            &mut header(info, EntryType::Regular, info.len()),
            path,
            &mut contents,
        )?;
        if contents.limit() != 0 {
            return Err(Error::Refused("A project file changed while it was read."));
        }
        unchanged(file, info)?;
        if info.nlink() > 1 {
            self.links.insert(identity, (path.to_owned(), info.clone()));
        }
        Ok(())
    }
}

fn symlink_entry(
    tar: &mut Builder<impl Write>,
    info: &Metadata,
    path: &Path,
    target: &[u8],
) -> io::Result<()> {
    let mut link = header(info, EntryType::Symlink, 0);
    if target.len() <= link.as_old().linkname.len() {
        link.set_link_name_literal(target)?;
    } else {
        let mut long = Header::new_gnu();
        long.set_entry_type(EntryType::GNULongLink);
        long.set_size(target.len() as u64 + 1);
        tar.append_data(&mut long, "@LongLink", target.chain(&[0_u8][..]))?;
    }
    tar.append_data(&mut link, path, io::empty())
}

fn header(info: &Metadata, kind: EntryType, size: u64) -> Header {
    let mut header = Header::new_gnu();
    header.set_entry_type(kind);
    header.set_size(size);
    header.set_mode(info.mode() & 0o777);
    header.set_uid(1000);
    header.set_gid(1000);
    header.set_mtime(u64::try_from(info.mtime()).unwrap_or_default());
    header
}

fn unchanged(file: &File, before: &Metadata) -> Result<()> {
    let after = file.metadata()?;
    if after.dev() != before.dev()
        || after.ino() != before.ino()
        || after.len() != before.len()
        || after.mode() != before.mode()
        || (after.mtime(), after.mtime_nsec()) != (before.mtime(), before.mtime_nsec())
        || (after.ctime(), after.ctime_nsec()) != (before.ctime(), before.ctime_nsec())
    {
        return Err(Error::Refused(
            "The stopped project changed during its checkpoint.",
        ));
    }
    Ok(())
}

pub(in crate::linux::workspace::handoff) fn local_link(path: &Path, target: &Path) -> bool {
    let mut parents = path.components().count().saturating_sub(1);
    let mut descended = false;
    for component in target.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir if !descended && parents > 0 => parents -= 1,
            Component::Normal(_) => descended = true,
            _ => return false,
        }
    }
    !target.as_os_str().is_empty()
}

#[cfg(test)]
mod tests;
