//! Record operations use the same descriptors that were checked for effective access.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, fchmod, fstat, fsync, mkdirat, openat, renameat, statat,
    unlinkat,
};
use rustix::io::Errno;

use super::{BadRecords, this_user};

const DIRECTORY: OFlags = OFlags::DIRECTORY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const FILE: OFlags = OFlags::NOFOLLOW
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
const ONLY_USER: Mode = Mode::RWXG.union(Mode::RWXO);
const WRITE_OTHERS: Mode = Mode::WGRP.union(Mode::WOTH);

/// A checked directory under the records root. State files are opened relative to it.
#[derive(Debug)]
pub(crate) struct Directory {
    fd: OwnedFd,
    root: PathBuf,
    path: PathBuf,
}

impl Directory {
    pub(crate) fn open(root: &Path, path: &Path, create: bool) -> io::Result<Self> {
        let parts = path.strip_prefix(root).map_err(io::Error::other)?;
        if !super::super::plain(parts) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut fd = root_folder(root, false).map_err(io::Error::from)?;
        for part in parts {
            fd = dir_at(&fd, part, create)?;
            owned(&fd, WRITE_OTHERS).map_err(io::Error::from)?;
        }
        let directory = Self {
            fd,
            root: root.to_owned(),
            path: path.to_owned(),
        };
        directory.check()?;
        Ok(directory)
    }

    fn check(&self) -> io::Result<()> {
        let held = fstat(&self.fd)?;
        let found = statat(rustix::fs::CWD, &self.path, AtFlags::SYMLINK_NOFOLLOW)?;
        if same(&held, &found) {
            Ok(())
        } else {
            Err(io::Error::other("the directory of the Git records changed"))
        }
    }

    /// Recheck access and identity before using a directory across a read or a mutation.
    fn current(&self) -> io::Result<&OwnedFd> {
        let now = Self::open(&self.root, &self.path, false)?;
        if same(&fstat(&self.fd)?, &fstat(&now.fd)?) {
            Ok(&self.fd)
        } else {
            Err(io::Error::other("the directory of the Git records changed"))
        }
    }

    pub(crate) fn names(&self) -> io::Result<Vec<OsString>> {
        super::super::walk::Dir::from_fd(self.current()?.try_clone()?).names()
    }

    pub(crate) fn read(&self, name: &OsStr) -> io::Result<Option<Vec<u8>>> {
        let Some(mut file) = self.file(name)? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(Some(bytes))
    }

    fn file(&self, name: &OsStr) -> io::Result<Option<File>> {
        let fd = match openat(self.current()?, name, FILE | OFlags::RDONLY, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        owned(&fd, WRITE_OTHERS).map_err(io::Error::from)?;
        let about = fstat(&fd)?;
        if FileType::from_raw_mode(about.st_mode) != FileType::RegularFile || about.st_nlink != 1 {
            return Err(io::Error::other(
                "a Git record is not a private regular file",
            ));
        }
        Ok(Some(fd.into()))
    }

    pub(crate) fn write(&self, name: &OsStr, bytes: &[u8]) -> io::Result<()> {
        self.file(name)?;
        let dir = self.current()?;
        let temporary = format!("record-{}.tmp", uuid::Uuid::now_v7().as_simple());
        let flags = FILE | OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL;
        let fd = openat(dir, temporary.as_str(), flags, Mode::RUSR | Mode::WUSR)?;
        let result = (|| {
            owned(&fd, ONLY_USER).map_err(io::Error::from)?;
            let mut file = File::from(fd);
            file.write_all(bytes)?;
            file.sync_all()?;
            self.file(name)?;
            let dir = self.current()?;
            renameat(dir, temporary.as_str(), dir, name)?;
            Ok(fsync(dir)?)
        })();
        let _ = unlinkat(dir, temporary.as_str(), AtFlags::empty());
        result
    }

    pub(crate) fn remove(&self, name: &OsStr) -> io::Result<()> {
        if self.file(name)?.is_some() {
            let dir = self.current()?;
            unlinkat(dir, name, AtFlags::empty())?;
            fsync(dir)?;
        }
        Ok(())
    }

    pub(crate) fn remove_tree(&self, name: &OsStr) -> io::Result<()> {
        let parent = self.current()?;
        match dir_at(parent, name, false) {
            Ok(child) => owned(&child, WRITE_OTHERS).map_err(io::Error::from)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
        super::super::walk::Dir::from_fd(parent.try_clone()?).remove_all(name)
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn archive(&self, name: &OsStr, saved: &OsStr) -> io::Result<()> {
        let dir = self.current()?;
        match dir_at(dir, name, false) {
            Ok(child) => owned(&child, WRITE_OTHERS).map_err(io::Error::from)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
        rustix::fs::renameat_with(dir, name, dir, saved, rustix::fs::RenameFlags::NOREPLACE)?;
        super::super::walk::sync_directory(dir)
    }

    pub(super) fn touch(&self, now: std::fs::FileTimes) -> io::Result<()> {
        let dir = self.current()?;
        for name in self.names()? {
            let result = openat(dir, &name, FILE | OFlags::RDONLY, Mode::empty());
            let Ok(fd) = result else {
                continue;
            };
            if owned(&fd, WRITE_OTHERS).is_err() {
                continue;
            }
            let stat = fstat(&fd)?;
            match FileType::from_raw_mode(stat.st_mode) {
                FileType::Directory => {
                    let child = Self {
                        fd,
                        path: self.path.join(name),
                        root: self.root.clone(),
                    };
                    child.touch(now)?;
                }
                FileType::RegularFile if stat.st_nlink == 1 => {
                    let _ = File::from(fd).set_times(now);
                }
                _ => {}
            }
        }
        File::from(self.current()?.try_clone()?).set_times(now)
    }
}

pub(super) fn root_folder(path: &Path, create: bool) -> Result<OwnedFd, BadRecords> {
    let parent = path.parent().ok_or(BadRecords::BadPath)?;
    if !path.is_absolute() || !super::super::plain(path) {
        return Err(BadRecords::BadPath);
    }
    let mut directory = openat(rustix::fs::CWD, "/", DIRECTORY, Mode::empty())
        .map_err(|_| BadRecords::OpenAbove)?;
    for part in parent.iter().skip(1) {
        ancestor(&directory)?;
        directory = dir_at(&directory, part, false).map_err(|_| BadRecords::OpenAbove)?;
    }
    ancestor(&directory)?;
    let name = path.file_name().ok_or(BadRecords::BadPath)?;
    let root = dir_at(&directory, name, create).map_err(|failed| match failed.raw_os_error() {
        Some(code) if code == Errno::NOENT.raw_os_error() => BadRecords::Gone,
        Some(code)
            if [Errno::NOTDIR, Errno::LOOP, Errno::MLINK]
                .iter()
                .any(|kind| kind.raw_os_error() == code) =>
        {
            BadRecords::NotAFolder
        }
        _ => BadRecords::Unreadable,
    })?;
    owned(&root, ONLY_USER)?;
    let observed = statat(rustix::fs::CWD, path, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| BadRecords::OpenAbove)?;
    let held = fstat(&root).map_err(|_| BadRecords::Unreadable)?;
    if !same(&held, &observed) {
        return Err(BadRecords::OpenAbove);
    }
    Ok(root)
}

fn dir_at(parent: &OwnedFd, name: &OsStr, create: bool) -> io::Result<OwnedFd> {
    let owner = Mode::RWXU;
    if create {
        match mkdirat(parent, name, owner) {
            Ok(()) => {
                let fd = openat(parent, name, DIRECTORY, Mode::empty())?;
                // Never chmod a node other than the checked, owner-only directory.
                owned(&fd, ONLY_USER).map_err(io::Error::from)?;
                fchmod(&fd, owner)?;
                return Ok(fd);
            }
            Err(Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(openat(parent, name, DIRECTORY, Mode::empty())?)
}

fn same(first: &rustix::fs::Stat, second: &rustix::fs::Stat) -> bool {
    first.st_dev == second.st_dev && first.st_ino == second.st_ino
}

fn ancestor(fd: &OwnedFd) -> Result<(), BadRecords> {
    let about = fstat(fd).map_err(|_| BadRecords::OpenAbove)?;
    let mode = Mode::from_raw_mode(about.st_mode);
    if (about.st_uid == 0 || about.st_uid == this_user())
        && (!mode.intersects(WRITE_OTHERS) || mode.contains(Mode::SVTX))
        && no_grants(fd).map_err(|_| BadRecords::OpenAbove)?
    {
        Ok(())
    } else {
        Err(BadRecords::OpenAbove)
    }
}

fn owned(fd: &OwnedFd, forbidden: Mode) -> Result<(), BadRecords> {
    let about = fstat(fd).map_err(|_| BadRecords::Unreadable)?;
    if about.st_uid != this_user() {
        return Err(BadRecords::NotOurs);
    }
    if Mode::from_raw_mode(about.st_mode).intersects(forbidden)
        || !no_grants(fd).map_err(|_| BadRecords::Unreadable)?
    {
        return Err(BadRecords::OpenToOthers);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn no_grants(fd: &OwnedFd) -> io::Result<bool> {
    use std::os::fd::AsFd as _;
    let acl = calcifer_macos_acl::read_acl(fd.as_fd())?;
    Ok(acl
        .entries
        .iter()
        .all(|entry| entry.tag == calcifer_macos_acl::TAG_DENY))
}

#[cfg(target_os = "linux")]
fn no_grants(fd: &OwnedFd) -> io::Result<bool> {
    for name in ["system.posix_acl_access", "system.posix_acl_default"] {
        match rustix::fs::fgetxattr_query(fd, name) {
            Ok(_) => return Ok(false),
            Err(Errno::NODATA | Errno::OPNOTSUPP) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn no_grants(_: &OwnedFd) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}
