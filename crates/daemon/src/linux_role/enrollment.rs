//! The root supervisor's restart-stable binding between enrollment and the person's daemon home.

use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read as _, Write as _},
    os::{fd::OwnedFd, unix::ffi::OsStrExt as _},
    path::{Path, PathBuf},
    sync::Mutex,
};

use rustix::{
    fs::{self, AtFlags, FileType, FlockOperation, Mode, OFlags, Stat},
    io::Errno,
};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    config::origin::{Origin, Reach},
    token_store::Credential,
};

#[cfg(test)]
mod tests;

const ENROLLMENT: &str = "enrollment";
const STAGING: &str = "enrollment.new";
const MARKER: &[u8] = b"ANIMA-TH32\x01";
const TOKEN_LIMIT: usize = 4096;
const RECORD_LIMIT: usize = MARKER.len() + 48 + 16 + 32 + TOKEN_LIMIT;
const DIRECTORY: OFlags = OFlags::DIRECTORY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(super) struct Enrollment {
    directory: OwnedFd,
    home: Home,
    access: Mutex<()>,
}

pub(super) struct Stored {
    pub(super) record: Zeroizing<Vec<u8>>,
    pub(super) seed: Zeroizing<[u8; 32]>,
    pub(super) device: Uuid,
    pub(super) hub: Origin,
}

struct Home {
    path: PathBuf,
    directory: OwnedFd,
    binding: [u8; 48],
}

impl Enrollment {
    pub(super) fn at(directory: &Path, home: PathBuf) -> io::Result<Self> {
        if !rustix::process::geteuid().is_root() {
            return Err(refused());
        }
        let home = Home::at(home)?;
        let directory = state_directory(directory)?;
        Ok(Self {
            directory,
            home,
            access: Mutex::new(()),
        })
    }

    pub(super) fn read(&self) -> io::Result<Option<Stored>> {
        let _access = self.access.lock().map_err(|_| refused())?;
        self.home.check()?;
        self.read_record()
    }

    fn read_record(&self) -> io::Result<Option<Stored>> {
        let flags =
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC;
        let fd = match fs::openat(&self.directory, ENROLLMENT, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let stat = fs::fstat(&fd)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_uid != 0
            || stat.st_mode & 0o7777 != 0o600
            || stat.st_nlink != 1
        {
            return Err(refused());
        }
        let length = usize::try_from(stat.st_size).map_err(|_| invalid())?;
        if length > RECORD_LIMIT {
            return Err(invalid());
        }
        let mut bytes = Zeroizing::new(vec![0; length]);
        let mut file = File::from(fd);
        file.read_exact(&mut bytes)?;
        if file.read(&mut *Zeroizing::new([0; 1]))? != 0 {
            return Err(invalid());
        }
        decode(&bytes, &self.home.binding).map(Some)
    }

    // The supervisor calls this with a device only for its owner's successful K8 spend.
    pub(super) fn save(&self, record: &[u8], device: Option<Uuid>) -> io::Result<()> {
        let _access = self.access.lock().map_err(|_| refused())?;
        self.home.check()?;
        let hub = record_hub(record)?;
        let (device, seed) = match device {
            Some(device) => (valid_device(device)?, new_seed()?),
            None => {
                let previous = self.read_record()?.ok_or_else(refused)?;
                if previous.hub != hub {
                    return Err(refused());
                }
                (previous.device, previous.seed)
            }
        };
        self.replace(&[
            MARKER,
            &self.home.binding,
            device.as_bytes(),
            &*seed,
            record,
        ])
    }

    pub(super) fn remove(&self) -> io::Result<()> {
        let _access = self.access.lock().map_err(|_| refused())?;
        let staging = remove(&self.directory, STAGING);
        remove(&self.directory, ENROLLMENT)?;
        fs::fsync(&self.directory)?;
        staging
    }

    fn replace(&self, pieces: &[&[u8]]) -> io::Result<()> {
        remove(&self.directory, STAGING)?;
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mode = Mode::RUSR | Mode::WUSR;
        let mut file = File::from(fs::openat(&self.directory, STAGING, flags, mode)?);
        let published = (|| {
            fs::fchmod(&file, mode)?;
            for bytes in pieces {
                file.write_all(bytes)?;
            }
            file.sync_all()?;
            self.home.check()?;
            fs::renameat(&self.directory, STAGING, &self.directory, ENROLLMENT)?;
            Ok(())
        })();
        if let Err(error) = published {
            remove(&self.directory, STAGING)?;
            return Err(error);
        }
        Ok(fs::fsync(&self.directory)?)
    }
}

impl Home {
    fn at(path: PathBuf) -> io::Result<Self> {
        let directory = open_home(&path)?;
        let stat = fs::fstat(&directory)?;
        let mut binding = [0; 48];
        binding[..32].copy_from_slice(&Sha256::digest(path.as_os_str().as_bytes()));
        binding[32..40].copy_from_slice(&stat.st_dev.to_be_bytes());
        binding[40..].copy_from_slice(&stat.st_ino.to_be_bytes());
        Ok(Self {
            path,
            directory,
            binding,
        })
    }

    fn check(&self) -> io::Result<()> {
        let expected = fs::fstat(&self.directory)?;
        let found = fs::fstat(open_home(&self.path)?)?;
        if (expected.st_dev, expected.st_ino) != (found.st_dev, found.st_ino) {
            return Err(refused());
        }
        Ok(())
    }
}

fn open_home(path: &Path) -> io::Result<OwnedFd> {
    let flags = DIRECTORY | OFlags::PATH;
    let mut directory = fs::open("/", flags, Mode::empty())?;
    for part in parts(path)? {
        directory = fs::openat(&directory, part, flags, Mode::empty())?;
    }
    Ok(directory)
}

fn state_directory(path: &Path) -> io::Result<OwnedFd> {
    let flags = DIRECTORY | OFlags::RDONLY;
    let mut directory = fs::open("/", flags, Mode::empty())?;
    let mut parts = parts(path)?.peekable();
    root_owned(&directory, false)?;
    while let Some(part) = parts.next() {
        let last = parts.peek().is_none();
        let child = match fs::openat(&directory, part, flags, Mode::empty()) {
            Err(Errno::NOENT) if last => make_state_directory(&directory, part, flags)?,
            opened => opened?,
        };
        root_owned(&child, last)?;
        directory = child;
    }
    fs::flock(&directory, FlockOperation::NonBlockingLockExclusive)?;
    Ok(directory)
}

fn make_state_directory(parent: &OwnedFd, name: &OsStr, flags: OFlags) -> io::Result<OwnedFd> {
    let mode = Mode::RUSR | Mode::WUSR | Mode::XUSR;
    match fs::mkdirat(parent, name, mode) {
        Ok(()) => fs::fsync(parent)?,
        Err(Errno::EXIST) => (),
        Err(error) => return Err(error.into()),
    }
    Ok(fs::openat(parent, name, flags, Mode::empty())?)
}

fn root_owned(directory: &OwnedFd, private: bool) -> io::Result<()> {
    let Stat {
        st_mode, st_uid, ..
    } = fs::fstat(directory)?;
    if st_uid != 0 || st_mode & 0o022 != 0 || (private && st_mode & 0o7777 != 0o700) {
        return Err(refused());
    }
    Ok(())
}

fn parts(path: &Path) -> io::Result<impl Iterator<Item = &OsStr>> {
    let path = path
        .as_os_str()
        .as_bytes()
        .strip_prefix(b"/")
        .ok_or_else(refused)?;
    let parts = path.split(|&byte| byte == b'/');
    if parts.clone().any(|part| matches!(part, b"" | b"." | b"..")) {
        return Err(refused());
    }
    Ok(parts.map(OsStr::from_bytes))
}

fn remove(directory: &OwnedFd, name: &str) -> io::Result<()> {
    match fs::unlinkat(directory, name, AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn decode(bytes: &[u8], binding: &[u8; 48]) -> io::Result<Stored> {
    let bytes = bytes.strip_prefix(MARKER).ok_or_else(invalid)?;
    let bytes = bytes.strip_prefix(binding).ok_or_else(refused)?;
    let (device, bytes) = bytes.split_at_checked(16).ok_or_else(invalid)?;
    let (seed, record) = bytes.split_at_checked(32).ok_or_else(invalid)?;
    let device = valid_device(Uuid::from_slice(device).map_err(|_| invalid())?)?;
    let hub = record_hub(record)?;
    let mut protected_seed = Zeroizing::new([0; 32]);
    protected_seed.copy_from_slice(seed);
    Ok(Stored {
        record: Zeroizing::new(record.to_vec()),
        seed: protected_seed,
        device,
        hub,
    })
}

fn record_hub(record: &[u8]) -> io::Result<Origin> {
    if record.len() > TOKEN_LIMIT {
        return Err(invalid());
    }
    let credential = Credential::from_record(record).map_err(|_| invalid())?;
    if !matches!(credential.hub.reach(), Reach::Https(_)) {
        return Err(invalid());
    }
    Ok(credential.hub)
}

fn valid_device(device: Uuid) -> io::Result<Uuid> {
    if device.is_nil() {
        return Err(invalid());
    }
    Ok(device)
}

fn new_seed() -> io::Result<Zeroizing<[u8; 32]>> {
    let mut seed = Zeroizing::new([0; 32]);
    getrandom::fill(seed.as_mut())
        .map_err(|_| io::Error::other("Linux enrollment randomness unavailable"))?;
    Ok(seed)
}

fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Linux enrollment binding refused",
    )
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid Linux enrollment record",
    )
}
