//! The supervisor serializes access to the enrollment's crash-durable Code journal.

use std::{
    fs::File,
    io::{self, Read as _, Write as _},
    os::fd::OwnedFd,
    sync::{Mutex, MutexGuard, TryLockError},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rustix::fs::{self, FileType, Mode, OFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use super::{Role, enrollment::Enrollment, exchange, protocol, refused};

pub(super) const LIMIT: usize = 8 * 1024 * 1024;
const PENDING_LIMIT: usize = 512;
const STAGING: &str = "journal.new";

#[derive(Deserialize, Serialize)]
pub(super) enum Request {
    Lock,
    Save(String),
    Unlock,
    Pending,
    SavePending(String),
}

#[derive(Deserialize, Serialize)]
pub(super) enum Reply {
    Locked(Option<String>),
    Busy,
    Done,
    Pending(Option<String>),
}

static CLIENT: Mutex<()> = Mutex::new(());

pub(crate) struct Locked {
    _guard: MutexGuard<'static, ()>,
}

impl Locked {
    pub(crate) fn take() -> io::Result<Option<(Self, Option<Zeroizing<String>>)>> {
        let guard = match CLIENT.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Poisoned(_)) => return Err(refused()),
        };
        match exchange(&protocol::Request::Journal(Request::Lock))? {
            protocol::Reply::Journal(Reply::Locked(json)) => {
                Ok(Some((Self { _guard: guard }, json.map(Zeroizing::new))))
            }
            protocol::Reply::Journal(Reply::Busy) => Ok(None),
            _ => Err(refused()),
        }
    }

    #[expect(
        clippy::unused_self,
        reason = "only the holder of this journal lock may write"
    )]
    pub(crate) fn save(&self, json: String) -> io::Result<()> {
        match exchange(&protocol::Request::Journal(Request::Save(json)))? {
            protocol::Reply::Journal(Reply::Done) => Ok(()),
            _ => Err(refused()),
        }
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        let _ = exchange(&protocol::Request::Journal(Request::Unlock));
    }
}

pub(crate) fn pending() -> io::Result<Option<String>> {
    match exchange(&protocol::Request::Journal(Request::Pending))? {
        protocol::Reply::Journal(Reply::Pending(record)) => Ok(record),
        _ => Err(refused()),
    }
}

pub(crate) fn save_pending(record: String) -> io::Result<()> {
    match exchange(&protocol::Request::Journal(Request::SavePending(record)))? {
        protocol::Reply::Journal(Reply::Done) => Ok(()),
        _ => Err(refused()),
    }
}

#[derive(Default)]
pub(super) struct Journal {
    owner: Option<Role>,
}

impl Journal {
    pub(super) fn release(&mut self, role: Role) {
        if self.owner == Some(role) {
            self.owner = None;
        }
    }

    pub(super) fn apply(
        &mut self,
        enrollment: &Enrollment,
        role: Role,
        request: Request,
    ) -> io::Result<Reply> {
        match request {
            Request::Lock if self.owner.is_none() => {
                let file = record(enrollment)?;
                let current = read(enrollment.directory(), &file, LIMIT)?;
                self.owner = Some(role);
                Ok(Reply::Locked(current))
            }
            Request::Lock => Ok(Reply::Busy),
            Request::Save(json) if self.owner == Some(role) => {
                replace(enrollment.directory(), &record(enrollment)?, &json, LIMIT)?;
                Ok(Reply::Done)
            }
            Request::Unlock if self.owner == Some(role) => {
                self.release(role);
                Ok(Reply::Done)
            }
            Request::Pending if role == Role::Holder => {
                let file = format!("{}-pending", record(enrollment)?);
                Ok(Reply::Pending(read(
                    enrollment.directory(),
                    &file,
                    PENDING_LIMIT,
                )?))
            }
            Request::SavePending(json) if role == Role::Holder => {
                let file = format!("{}-pending", record(enrollment)?);
                replace(enrollment.directory(), &file, &json, PENDING_LIMIT)?;
                Ok(Reply::Done)
            }
            _ => Err(refused()),
        }
    }
}

fn record(enrollment: &Enrollment) -> io::Result<String> {
    let stored = enrollment.read()?.ok_or_else(refused)?;
    Ok(format!(
        "journal-{}",
        URL_SAFE_NO_PAD.encode(Sha256::digest(stored.seed.as_ref()))
    ))
}

fn read(directory: &OwnedFd, name: &str, limit: usize) -> io::Result<Option<String>> {
    let fd = match fs::openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = fs::fstat(&fd)?;
    if metadata.st_uid != 0
        || metadata.st_mode & 0o7777 != 0o600
        || metadata.st_nlink != 1
        || FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile
    {
        return Err(refused());
    }
    let mut text = String::new();
    File::from(fd)
        .take((limit + 1) as u64)
        .read_to_string(&mut text)?;
    if text.len() > limit {
        return Err(refused());
    }
    Ok(Some(text))
}

fn replace(directory: &OwnedFd, name: &str, json: &str, limit: usize) -> io::Result<()> {
    if json.is_empty() || json.len() > limit {
        return Err(refused());
    }
    match fs::unlinkat(directory, STAGING, fs::AtFlags::empty()) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => {}
        Err(error) => return Err(error.into()),
    }
    let mode = Mode::RUSR | Mode::WUSR;
    let mut file = File::from(fs::openat(
        directory,
        STAGING,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        mode,
    )?);
    fs::fchmod(&file, mode)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    fs::renameat(directory, STAGING, directory, name)?;
    Ok(fs::fsync(directory)?)
}
