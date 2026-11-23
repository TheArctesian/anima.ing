//! The unpack of a runtime archive (Section 6, F10). Each file and folder is made relative to an
//! open handle of its folder with `O_NOFOLLOW`; only regular files, folders and the links that
//! [`links`] turns into copies; no hard link, device, FIFO, absolute path or `..`. Folders and
//! program files get 0755, other files 0644, with no setuid, setgid or sticky bit. Blocking: the
//! caller runs it on a blocking thread.

mod links;
mod tar_gz;
mod zip;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::ArchiveFormat;
use rustix::fs::{Mode, OFlags};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: u64,
    pub entries: usize,
}

impl Limits {
    pub const SECTION_6: Self = Self {
        bytes: 8 << 30,
        entries: 50_000,
    };
}

const BAD_ARCHIVE: Cause = Cause::ArchiveRejected;

const DIR_FLAGS: OFlags = OFlags::DIRECTORY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::RDONLY)
    .union(OFlags::CLOEXEC);

#[derive(Clone, Copy)]
enum Kind {
    Folder,
    /// A regular file with the mode bits of the archive.
    File(u32),
}

fn open_dir(parent: &OwnedFd, name: &str, create: bool) -> io::Result<OwnedFd> {
    let made = create
        && match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o755)) {
            Ok(()) => true,
            Err(rustix::io::Errno::EXIST) => false,
            Err(error) => return Err(error.into()),
        };
    let dir = rustix::fs::openat(parent, name, DIR_FLAGS, Mode::empty())?;
    if made {
        // The mode of `mkdirat` passes through the umask; the contract wants 0755 exactly.
        rustix::fs::fchmod(&dir, Mode::from_raw_mode(0o755))?;
    }
    Ok(dir)
}

fn names(path: &str) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for part in Path::new(path).components() {
        match part {
            Component::Normal(name) => {
                let name = name.to_str()?;
                if name.contains('\\') {
                    return None;
                }
                names.push(name.to_owned());
            }
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    (!names.is_empty()).then_some(names)
}

fn own_empty_folder(root: &OwnedFd) -> bool {
    let owned = rustix::fs::fstat(root)
        .is_ok_and(|stat| stat.st_uid == rustix::process::geteuid().as_raw());
    let Ok(mut entries) = rustix::fs::Dir::read_from(root) else {
        return false;
    };
    let empty = entries
        .all(|entry| entry.is_ok_and(|entry| matches!(entry.file_name().to_bytes(), b"." | b"..")));
    owned && empty
}

struct Writer<F: Fn() -> bool> {
    root: OwnedFd,
    /// Each path seen, and each of its parents, by its name in lower case.
    seen: HashMap<String, String>,
    files: HashMap<String, u32>,
    links: Vec<(Vec<String>, String)>,
    left: u64,
    entries: usize,
    max_entries: usize,
    disk_holds: F,
}

impl<F: Fn() -> bool> Writer<F> {
    fn new(into: &Path, limits: Limits, disk_holds: F) -> Result<Self, Cause> {
        // A folder that does not open, is a link, is not this user's or is not empty is a fault
        // of the order, not of the disk.
        let root = rustix::fs::open(into, DIR_FLAGS, Mode::empty())
            .map_err(|_| Cause::RuntimeNotConfigured)?;
        if !own_empty_folder(&root) {
            return Err(Cause::RuntimeNotConfigured);
        }
        Ok(Self {
            root,
            seen: HashMap::new(),
            files: HashMap::new(),
            links: Vec::new(),
            left: limits.bytes,
            entries: 0,
            max_entries: limits.entries,
            disk_holds,
        })
    }

    /// On a volume that ignores case, two names equal in lower case would be one folder or file.
    /// A parent, or a folder entry, may come again with the same spelling.
    fn first_seen(&mut self, names: &[String], folder: bool) -> bool {
        for end in 1..=names.len() {
            let path = names[..end].join("/");
            let last = end == names.len();
            match self.seen.get(&path.to_lowercase()) {
                None => {
                    self.seen.insert(path.to_lowercase(), path);
                }
                Some(known) if *known == path && (!last || folder) => {}
                Some(_) => return false,
            }
        }
        true
    }

    fn next_entry(&mut self) -> Result<(), Cause> {
        self.entries += 1;
        if self.entries > self.max_entries || !(self.disk_holds)() {
            return Err(Cause::LimitDisk);
        }
        Ok(())
    }

    fn write(&mut self, path: &str, kind: Kind, bytes: &mut dyn Read) -> Result<(), Cause> {
        let names = self.claim(path, matches!(kind, Kind::Folder))?;
        self.place(&names, kind, bytes)
    }

    fn claim(&mut self, path: &str, folder: bool) -> Result<Vec<String>, Cause> {
        let names = names(path).ok_or(BAD_ARCHIVE)?;
        if !self.first_seen(&names, folder) {
            return Err(BAD_ARCHIVE);
        }
        Ok(names)
    }

    fn place(&mut self, names: &[String], kind: Kind, bytes: &mut dyn Read) -> Result<(), Cause> {
        let (last, parents) = names.split_last().ok_or(BAD_ARCHIVE)?;
        let mut dir = rustix::fs::openat(&self.root, ".", DIR_FLAGS, Mode::empty())
            .map_err(|_| Cause::LimitDisk)?;
        for name in parents {
            dir = open_dir(&dir, name, true).map_err(|_| BAD_ARCHIVE)?;
        }
        match kind {
            Kind::Folder => drop(open_dir(&dir, last, true).map_err(|_| BAD_ARCHIVE)?),
            Kind::File(mode) => {
                self.write_file(&dir, (last, mode), bytes)?;
                self.files.insert(names.join("/"), mode);
            }
        }
        Ok(())
    }

    fn write_file(
        &mut self,
        dir: &OwnedFd,
        (name, mode): (&str, u32),
        bytes: &mut dyn Read,
    ) -> Result<(), Cause> {
        let flags =
            OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mode = Mode::from_raw_mode(if mode & 0o111 != 0 { 0o755 } else { 0o644 });
        let fd = rustix::fs::openat(dir, name, flags, mode).map_err(|_| BAD_ARCHIVE)?;
        rustix::fs::fchmod(&fd, mode).map_err(|_| Cause::LimitDisk)?;
        self.copy(bytes, &mut File::from(fd))
    }

    /// The disk is asked again after each [`DISK_CHECK_BYTES`], so one large entry cannot pass
    /// the reserve.
    fn copy(&mut self, bytes: &mut dyn Read, file: &mut File) -> Result<(), Cause> {
        use io::Write as _;
        let mut buffer = vec![0_u8; 1 << 16];
        let mut since_check = 0_u64;
        loop {
            let read = match bytes.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(BAD_ARCHIVE),
            };
            let read_bytes = read as u64;
            if read_bytes > self.left {
                return Err(Cause::LimitDisk);
            }
            file.write_all(&buffer[..read])
                .map_err(|_| Cause::LimitDisk)?;
            self.left -= read_bytes;
            since_check += read_bytes;
            if since_check >= DISK_CHECK_BYTES {
                since_check = 0;
                if !(self.disk_holds)() {
                    return Err(Cause::LimitDisk);
                }
            }
        }
    }
}

const DISK_CHECK_BYTES: u64 = 64 << 20;

/// `disk_holds` says whether the free disk is still above the reserve.
///
/// # Errors
/// As [`unpack_within`].
pub fn unpack(
    archive: File,
    format: ArchiveFormat,
    into: &Path,
    disk_holds: impl Fn() -> bool,
) -> Result<(), Cause> {
    unpack_within(archive, (format, Limits::SECTION_6), into, disk_holds)
}

/// # Errors
/// `archive_rejected` for an entry that breaks a rule, also a link that names no file of the
/// archive; `limit_disk` past a limit, below the reserve, or for a write that fails.
pub fn unpack_within(
    archive: File,
    (format, limits): (ArchiveFormat, Limits),
    into: &Path,
    disk_holds: impl Fn() -> bool,
) -> Result<(), Cause> {
    let mut writer = Writer::new(into, limits, disk_holds)?;
    match format {
        ArchiveFormat::TarGz => tar_gz::unpack(archive, &mut writer),
        ArchiveFormat::Zip => zip::unpack(archive, &mut writer),
    }?;
    writer.finish_links()
}
