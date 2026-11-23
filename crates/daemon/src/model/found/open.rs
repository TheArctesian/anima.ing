//! The opens of the folders of Section 10.2: each relative to an open handle, with `O_NOFOLLOW`,
//! `O_NONBLOCK` and no write access, and only of an entry that the owner rule permits
//! (`contracts/compute/model-work.md` Section 2.3). Nothing here writes.

use std::fs::{File, Metadata};
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::found::{MAX_ENTRIES, is_part, owner_permits};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};
use sha2::{Digest as _, Sha256};

fn folder_flags() -> OFlags {
    OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC
}

fn file_flags() -> OFlags {
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
}

fn kind(stat: &Stat) -> FileType {
    FileType::from_raw_mode(stat.st_mode)
}

fn others_write(stat: &Stat) -> bool {
    Mode::from_raw_mode(stat.st_mode).intersects(Mode::WGRP | Mode::WOTH)
}

/// A folder of Section 10.2, or a folder below one, open to read.
pub struct Folder {
    fd: OwnedFd,
    /// The owner of the folder of Section 10.2 that holds it.
    owner: u32,
}

/// A regular file open to read, with what `fstat` said of it.
pub struct Opened {
    pub fd: OwnedFd,
    pub meta: Metadata,
}

impl Folder {
    /// The folder at `path`, when it is there and the owner rule permits it.
    #[must_use]
    pub fn root(path: &Path) -> Option<Self> {
        let fd = rustix::fs::open(path, folder_flags(), Mode::empty()).ok()?;
        let stat = rustix::fs::fstat(&fd).ok()?;
        let daemon = rustix::process::geteuid().as_raw();
        let permits = owner_permits(stat.st_uid, others_write(&stat), daemon, stat.st_uid);
        (kind(&stat) == FileType::Directory && permits).then_some(Self {
            fd,
            owner: stat.st_uid,
        })
    }

    fn permits(&self, stat: &Stat) -> bool {
        let daemon = rustix::process::geteuid().as_raw();
        owner_permits(stat.st_uid, others_write(stat), daemon, self.owner)
    }

    /// The folder `name` in this folder; `None` for a link or any other entry.
    #[must_use]
    pub fn folder(&self, name: &str) -> Option<Self> {
        let fd = rustix::fs::openat(&self.fd, name, folder_flags(), Mode::empty()).ok()?;
        let stat = rustix::fs::fstat(&fd).ok()?;
        (kind(&stat) == FileType::Directory && self.permits(&stat)).then_some(Self {
            fd,
            owner: self.owner,
        })
    }

    /// The size of the regular file `name`, with no link followed; `None` for each other entry.
    #[must_use]
    pub fn file_size(&self, name: &str) -> Option<u64> {
        let stat = rustix::fs::statat(&self.fd, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
        (kind(&stat) == FileType::RegularFile && self.permits(&stat))
            .then(|| stat.st_size.cast_unsigned())
    }

    /// The text of the link `name`; `None` when it is not a link.
    #[must_use]
    pub fn link(&self, name: &str) -> Option<Vec<u8>> {
        let text = rustix::fs::readlinkat(&self.fd, name, Vec::new()).ok()?;
        Some(text.into_bytes())
    }

    /// The flat names in this folder, sorted; none when it has more than [`MAX_ENTRIES`] entries.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let Ok(entries) = rustix::fs::Dir::read_from(&self.fd) else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for entry in entries.take(MAX_ENTRIES + 2) {
            let Ok(entry) = entry else { return Vec::new() };
            if let Ok(name) = entry.file_name().to_str()
                && is_part(name)
            {
                names.push(name.to_owned());
            }
        }
        if names.len() > MAX_ENTRIES {
            return Vec::new();
        }
        names.sort_unstable();
        names
    }

    /// Opens the regular file `name` to read.
    ///
    /// # Errors
    /// `copy_missing` when it is not there, is not a regular file or the owner rule refuses it.
    pub fn file(&self, name: &str) -> Result<Opened, Cause> {
        let fd = rustix::fs::openat(&self.fd, name, file_flags(), Mode::empty())
            .map_err(|_| Cause::CopyMissing)?;
        let file = File::from(fd);
        let meta = file.metadata().map_err(|_| Cause::CopyMissing)?;
        let daemon = rustix::process::geteuid().as_raw();
        let others_write = meta.mode() & 0o022 != 0;
        if meta.is_file() && owner_permits(meta.uid(), others_write, daemon, self.owner) {
            Ok(Opened {
                fd: file.into(),
                meta,
            })
        } else {
            Err(Cause::CopyMissing)
        }
    }

    /// The bytes of the small regular file `name`; `None` when it has more than `most` bytes.
    #[must_use]
    pub fn small_file(&self, name: &str, most: usize) -> Option<Vec<u8>> {
        let opened = self.file(name).ok()?;
        let size = usize::try_from(opened.meta.len()).ok()?;
        if size > most {
            return None;
        }
        let mut bytes = vec![0u8; size];
        let read = rustix::io::pread(&opened.fd, &mut bytes, 0).ok()?;
        bytes.truncate(read);
        Some(bytes)
    }

    /// The folder at the parts of `location` below this folder, each opened by the rules above.
    #[must_use]
    pub fn below<'a>(&self, location: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let mut parts = location.into_iter();
        let first = parts.next()?;
        let mut folder = self.folder(first)?;
        for part in parts {
            folder = folder.folder(part)?;
        }
        Some(folder)
    }
}

/// The size and the SHA-256 of an open file, read from byte 0 with `pread`, so the offset of the
/// descriptor stays where it was.
///
/// # Errors
/// An I/O error of a read.
pub fn size_and_sha256(fd: &OwnedFd) -> io::Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut size = 0u64;
    loop {
        let read = match rustix::io::pread(fd, &mut buffer, size) {
            Ok(0) => break,
            Ok(read) => read,
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        };
        hash.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((size, crate::model::fetch::hex(&hash.finalize())))
}
