//! Moves or verifies a retained worktree relative to pinned, private directories.

use std::ffi::OsStr;
use std::io::{self, Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{
    AtFlags, Mode, OFlags, RenameFlags, fstat, mkdirat, open, openat, renameat_with, statat,
};

use super::{FolderId, RETAINED, Record};

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

struct Destination {
    home: OwnedFd,
    retired: OwnedFd,
    folder: OwnedFd,
    path: PathBuf,
}

impl Destination {
    fn open(home: &Path, container: &Path, create: bool) -> io::Result<Self> {
        let root = open(home, DIRECTORY, Mode::empty())?;
        private(&root, Mode::WGRP | Mode::WOTH)?;
        at_path(&root, home)?;
        let retired = child(&root, RETAINED.as_ref(), create)?;
        private(&retired, Mode::RWXG | Mode::RWXO)?;
        let name = container.file_name().ok_or(io::ErrorKind::InvalidInput)?;
        let folder = child(&retired, name, create)?;
        let destination = Self {
            home: root,
            retired,
            folder,
            path: container.to_owned(),
        };
        destination.check()?;
        Ok(destination)
    }

    fn check(&self) -> io::Result<()> {
        let retired = self.path.parent().ok_or(io::ErrorKind::InvalidInput)?;
        let home = retired.parent().ok_or(io::ErrorKind::InvalidInput)?;
        private(&self.home, Mode::WGRP | Mode::WOTH)?;
        private(&self.retired, Mode::RWXG | Mode::RWXO)?;
        private(&self.folder, Mode::RWXG | Mode::RWXO)?;
        at_path(&self.home, home)?;
        at_path(&self.retired, retired)?;
        at_path(&self.folder, &self.path)
    }

    fn worktree(&self) -> io::Result<Option<FolderId>> {
        identity(&self.folder, "worktree".as_ref())
    }

    fn sync(&self) -> io::Result<()> {
        for dir in [&self.folder, &self.retired, &self.home] {
            rustix::fs::fsync(dir)?;
        }
        Ok(())
    }
}

/// Verify an already retired folder without creating directories or following links.
pub(super) fn verify(home: &Path, container: &Path, original: FolderId) -> io::Result<()> {
    let destination = Destination::open(home, container, false)?;
    require(destination.worktree()?, original)?;
    destination.check()
}

/// Move the original directory. If the move already completed, verify the same private
/// ancestry before accepting the previous result.
pub(super) fn retire(record: &Record, container: &Path, original: FolderId) -> io::Result<()> {
    let destination = Destination::open(&record.home, container, true)?;
    keep_origin(&destination.folder, record)?;
    if let Some(retired) = destination.worktree()? {
        require(Some(retired), original)?;
        destination.check()?;
        return destination.sync();
    }
    let source = record.folder.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let parent_name = source.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let parent = openat(&destination.home, parent_name, DIRECTORY, Mode::empty())?;
    at_path(&parent, source)?;
    let name = record
        .folder
        .file_name()
        .ok_or(io::ErrorKind::InvalidInput)?;
    require(identity(&parent, name)?, original)?;
    destination.check()?;
    renameat_with(
        &parent,
        name,
        &destination.folder,
        "worktree",
        RenameFlags::NOREPLACE,
    )
    .map_err(move_error)?;
    let checked = destination
        .worktree()
        .and_then(|found| require(found, original))
        .and_then(|()| destination.check());
    if let Err(error) = checked {
        // A failed check never replaces anything that appeared at the source.
        renameat_with(
            &destination.folder,
            "worktree",
            &parent,
            name,
            RenameFlags::NOREPLACE,
        )
        .map_err(move_error)?;
        return Err(error);
    }
    rustix::fs::fsync(&parent)?;
    destination.sync()
}

/// Keep enough information beside the recovery tree to find native Git metadata if the
/// daemon's own records expire. An existing file must describe this exact origin.
fn keep_origin(folder: &OwnedFd, record: &Record) -> io::Result<()> {
    const ORIGIN: &str = "original.json";
    let bytes = serde_json::to_vec(&serde_json::json!({
        "original_worktree": record.folder,
        "linked_git_dir": record.git_dir,
    }))?;
    let temporary = format!("origin-{}.tmp", uuid::Uuid::now_v7().as_simple());
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC;
    let mut file = std::fs::File::from(openat(
        folder,
        temporary.as_str(),
        flags,
        Mode::RUSR | Mode::WUSR,
    )?);
    let written = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| {
        match renameat_with(
            folder,
            temporary.as_str(),
            folder,
            ORIGIN,
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => Ok(()),
            Err(rustix::io::Errno::EXIST) => same_origin(folder, ORIGIN, &bytes),
            Err(error) => Err(error.into()),
        }
    });
    // Only this call's exclusive temporary name can be left over.
    let _ = rustix::fs::unlinkat(folder, temporary.as_str(), AtFlags::empty());
    result?;
    Ok(rustix::fs::fsync(folder)?)
}

fn same_origin(folder: &OwnedFd, name: &str, expected: &[u8]) -> io::Result<()> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let file = std::fs::File::from(openat(folder, name, flags, Mode::empty())?);
    let mut actual = Vec::new();
    let limit = u64::try_from(expected.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    if file.metadata()?.is_file() {
        file.take(limit).read_to_end(&mut actual)?;
    }
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(
            "the recovery directory already has a different original location",
        ))
    }
}

/// Vacates an active Git directory without replacing a recovery entry.
pub(super) fn archive(source: &Path, saved: &OsStr) -> io::Result<()> {
    let parent = source.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let dir = open(parent, DIRECTORY, Mode::empty())?;
    at_path(&dir, parent)?;
    let name = source.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    if identity(&dir, name)?.is_some() {
        renameat_with(&dir, name, &dir, saved, RenameFlags::NOREPLACE).map_err(move_error)?;
    }
    Ok(())
}

fn identity(parent: &OwnedFd, name: &OsStr) -> io::Result<Option<FolderId>> {
    match openat(parent, name, DIRECTORY, Mode::empty()) {
        Ok(fd) => FolderId::from_directory(fd).map(Some),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn require(found: Option<FolderId>, original: FolderId) -> io::Result<()> {
    if found == Some(original) {
        Ok(())
    } else {
        Err(io::Error::other("the original worktree directory changed"))
    }
}

fn move_error(error: rustix::io::Errno) -> io::Error {
    if error == rustix::io::Errno::XDEV {
        io::Error::other("the recovery folder is on another filesystem")
    } else {
        error.into()
    }
}

fn child(parent: &OwnedFd, name: &OsStr, create: bool) -> io::Result<OwnedFd> {
    if create {
        match mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(openat(parent, name, DIRECTORY, Mode::empty())?)
}

fn private(dir: &OwnedFd, disallowed: Mode) -> io::Result<()> {
    let stat = fstat(dir)?;
    if stat.st_uid == rustix::process::getuid().as_raw()
        && !Mode::from_raw_mode(stat.st_mode).intersects(disallowed)
        && no_acl(dir)?
    {
        Ok(())
    } else {
        Err(io::Error::other(
            "the recovery directory ancestry is not private to this user",
        ))
    }
}

#[cfg(target_os = "macos")]
fn no_acl(dir: &OwnedFd) -> io::Result<bool> {
    use std::os::fd::AsFd as _;
    Ok(calcifer_macos_acl::read_acl(dir.as_fd())?.is_empty())
}

#[cfg(target_os = "linux")]
fn no_acl(dir: &OwnedFd) -> io::Result<bool> {
    match rustix::fs::fgetxattr_query(dir, "system.posix_acl_access") {
        Ok(_) => Ok(false),
        Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => Ok(true),
        Err(error) => Err(error.into()),
    }
}

fn at_path(dir: &OwnedFd, path: &Path) -> io::Result<()> {
    let expected = fstat(dir)?;
    let actual = statat(rustix::fs::CWD, path, AtFlags::SYMLINK_NOFOLLOW)?;
    if expected.st_dev == actual.st_dev
        && expected.st_ino == actual.st_ino
        && descriptor_path(dir)? == path
    {
        Ok(())
    } else {
        Err(io::Error::other("the recovery directory ancestry changed"))
    }
}

#[cfg(target_os = "macos")]
fn descriptor_path(dir: &OwnedFd) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt as _;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(
        rustix::fs::getpath(dir)?.into_bytes(),
    )))
}

#[cfg(target_os = "linux")]
fn descriptor_path(dir: &OwnedFd) -> io::Result<PathBuf> {
    use std::os::fd::AsRawFd as _;
    std::fs::read_link(format!("/proc/self/fd/{}", dir.as_raw_fd()))
}
