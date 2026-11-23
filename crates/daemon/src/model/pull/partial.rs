//! The files of the model cache, each relative to an open handle of its folder and with no link
//! followed (Section 7, Section 10, SEC-C-012).

use std::ffi::CStr;
use std::fs::File;
use std::io::{self, Read as _};
use std::os::fd::{AsFd, OwnedFd};
use std::time::{Duration, SystemTime};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
use sha2::{Digest as _, Sha256};

const FILE_MODE: u32 = 0o600;

/// The deepest folder that a removal goes into; a copy holds only flat files.
const MAX_DEPTH: usize = 4;

fn refused(text: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, text)
}

/// A regular file of this user with one name: no link, no device, no second hard link.
fn own_plain_file(stat: &Stat) -> io::Result<()> {
    let plain = FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile;
    let own = stat.st_uid == rustix::process::geteuid().as_raw();
    if plain && own && stat.st_nlink == 1 {
        Ok(())
    } else {
        Err(refused(
            "a file of the model cache is not a plain file of this user",
        ))
    }
}

/// Opens the partial file `name` of a copy to write: a new one with `O_EXCL`, else the one that
/// an earlier pull left, never through a link (R-408).
///
/// # Errors
/// An I/O error, or `PermissionDenied` for a file that is not a plain file of this user.
pub fn open_partial(folder: &OwnedFd, name: &str) -> io::Result<File> {
    // `NONBLOCK` so a FIFO in place of the file cannot hold the open; it is refused below.
    let base = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let mode = Mode::from_raw_mode(FILE_MODE);
    let fd = match rustix::fs::openat(folder, name, base | OFlags::CREATE | OFlags::EXCL, mode) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::EXIST) => rustix::fs::openat(folder, name, base, Mode::empty())?,
        Err(error) => return Err(error.into()),
    };
    own_plain_file(&rustix::fs::fstat(&fd)?)?;
    Ok(File::from(fd))
}

/// The size and the SHA-256 of the file `name`, read from byte 0 (SF-357).
///
/// # Errors
/// An I/O error, or `PermissionDenied` for a file that is not a plain file of this user.
pub fn size_and_sha256(folder: &OwnedFd, name: &str) -> io::Result<(u64, String)> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(folder, name, flags, Mode::empty())?;
    own_plain_file(&rustix::fs::fstat(&fd)?)?;
    let mut file = File::from(fd);
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut size = 0u64;
    loop {
        let read = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        hash.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((size, crate::model::fetch::hex(&hash.finalize())))
}

/// # Errors
/// An I/O error of `unlinkat`.
pub fn delete_file(folder: &OwnedFd, name: &str) -> io::Result<()> {
    rustix::fs::unlinkat(folder, name, AtFlags::empty()).map_err(Into::into)
}

/// Moves the folder `name` from `from` to `to`; an entry of that name in `to` is not replaced.
///
/// # Errors
/// An I/O error; `AlreadyExists` when `to` has `name`.
pub fn move_folder(from: &OwnedFd, to: &OwnedFd, name: &str) -> io::Result<()> {
    rustix::fs::renameat_with(from, name, to, name, RenameFlags::NOREPLACE).map_err(Into::into)
}

/// Removes each entry of `folder` whose name is not in `kept`, so only the files of the order
/// move to `copies/` (PULL-005).
///
/// # Errors
/// An I/O error of the folder or of a removal.
pub fn keep_only(folder: &OwnedFd, kept: &[&str]) -> io::Result<()> {
    for entry in names(folder)? {
        if !kept.iter().any(|name| name.as_bytes() == entry.as_bytes()) {
            remove_below(folder, &entry, 1)?;
        }
    }
    Ok(())
}

/// Whether `parent` has a folder `name` of this user; a link or any other entry is an error.
///
/// # Errors
/// An I/O error, or `PermissionDenied` for an entry that is not a folder of this user.
pub fn own_folder(parent: &OwnedFd, name: &CStr) -> io::Result<bool> {
    let stat = match rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let folder = FileType::from_raw_mode(stat.st_mode) == FileType::Directory;
    if folder && stat.st_uid == rustix::process::geteuid().as_raw() {
        Ok(true)
    } else {
        Err(refused(
            "an entry of the model cache is not a folder of this user",
        ))
    }
}

/// The bytes of the files in the folder `name` of `parent`; 0 when it cannot be read.
#[must_use]
pub fn bytes_in(parent: &OwnedFd, name: &CStr) -> u64 {
    sizes(parent, name).map_or(0, |(bytes, _)| bytes)
}

fn open_folder(parent: impl AsFd, name: &CStr) -> rustix::io::Result<OwnedFd> {
    let flags = OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::RDONLY | OFlags::CLOEXEC;
    rustix::fs::openat(parent, name, flags, Mode::empty())
}

/// The names in a folder, without `.` and `..`.
fn names(folder: &OwnedFd) -> io::Result<Vec<std::ffi::CString>> {
    let mut list = Vec::new();
    for entry in rustix::fs::Dir::read_from(folder)? {
        let name = entry?.file_name().to_owned();
        if name.as_bytes() != b"." && name.as_bytes() != b".." {
            list.push(name);
        }
    }
    Ok(list)
}

/// Removes `name` in `parent` and, for a folder, each entry in it first. A link is removed as a
/// link: nothing it names is touched (Section 10). A name that is not there is no error.
///
/// # Errors
/// An I/O error, or `InvalidInput` for folders deeper than the cache has.
pub fn remove_below(parent: &OwnedFd, name: &CStr, depth: usize) -> io::Result<()> {
    let folder = match open_folder(parent, name) {
        Ok(folder) => folder,
        Err(rustix::io::Errno::NOENT) => return Ok(()),
        Err(rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
            return rustix::fs::unlinkat(parent, name, AtFlags::empty()).map_err(Into::into);
        }
        Err(error) => return Err(error.into()),
    };
    if depth >= MAX_DEPTH {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "too deep"));
    }
    for entry in names(&folder)? {
        remove_below(&folder, &entry, depth + 1)?;
    }
    rustix::fs::unlinkat(parent, name, AtFlags::REMOVEDIR).map_err(Into::into)
}

/// The partial copies in `partial` other than `own`, each with its bytes and the time since a
/// pull last wrote to it.
pub struct Partial {
    pub name: std::ffi::CString,
    pub bytes: u64,
    pub age: Duration,
}

/// # Errors
/// An I/O error of the folder.
pub fn partial_copies(partial: &OwnedFd, now: SystemTime) -> io::Result<Vec<Partial>> {
    names(partial)?
        .into_iter()
        .map(|name| {
            let (bytes, newest) = sizes(partial, &name)?;
            let age = now.duration_since(newest).unwrap_or_default();
            Ok(Partial { name, bytes, age })
        })
        .collect()
}

fn modified(stat: &Stat) -> SystemTime {
    let seconds = u64::try_from(stat.st_mtime).unwrap_or_default();
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

/// The bytes of the files in the folder `name` and the newest change of it or of a file in it;
/// a name that is not a folder counts as its own size.
fn sizes(parent: &OwnedFd, name: &CStr) -> io::Result<(u64, SystemTime)> {
    let own = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    let Ok(folder) = open_folder(parent, name) else {
        return Ok((own.st_size.cast_unsigned(), modified(&own)));
    };
    let mut total = (0u64, modified(&own));
    for entry in names(&folder)? {
        let stat = rustix::fs::statat(&folder, &entry, AtFlags::SYMLINK_NOFOLLOW)?;
        total.0 = total.0.saturating_add(stat.st_size.cast_unsigned());
        total.1 = total.1.max(modified(&stat));
    }
    Ok(total)
}
