//! The records of the found copies in `<model-state>/found/` (Section 2.3, Section 10): written
//! with `O_EXCL` and mode 0600, and read back as untrusted.

use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::found::record::Record;
use rustix::fs::{AtFlags, Mode, OFlags};
use uuid::Uuid;

use crate::model::folders::private_below;

/// The most bytes of one record.
const MAX_RECORD: u64 = 1 << 20;

fn folder(state: &Path) -> Result<OwnedFd, Cause> {
    private_below(state, Path::new("found")).map_err(|_| Cause::LimitDisk)
}

fn name_of(copy: Uuid) -> String {
    format!("{copy}.json")
}

/// Writes the record to a new file and moves it over the record of an earlier attempt.
///
/// # Errors
/// `limit_disk` when the state folder cannot be written.
pub fn write(state: &Path, record: &Record) -> Result<(), Cause> {
    let folder = folder(state)?;
    let text = serde_json::to_vec(record).map_err(|_| Cause::LimitDisk)?;
    let temporary = format!(".{}", name_of(record.copy));
    let _ = rustix::fs::unlinkat(&folder, temporary.as_str(), AtFlags::empty());
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(
        &folder,
        temporary.as_str(),
        flags,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| Cause::LimitDisk)?;
    let mut file = File::from(fd);
    file.write_all(&text)
        .and_then(|()| file.sync_all())
        .map_err(|_| Cause::LimitDisk)?;
    rustix::fs::renameat(
        &folder,
        temporary.as_str(),
        &folder,
        name_of(record.copy).as_str(),
    )
    .map_err(|_| Cause::LimitDisk)
}

/// The record of `copy`.
///
/// # Errors
/// `copy_unknown` when there is none, or it is not a plain file of this user of the form that
/// the daemon writes.
pub fn read(state: &Path, copy: Uuid) -> Result<Record, Cause> {
    let folder = folder(state).map_err(|_| Cause::CopyUnknown)?;
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(&folder, name_of(copy).as_str(), flags, Mode::empty())
        .map_err(|_| Cause::CopyUnknown)?;
    let file = File::from(fd);
    let meta = file.metadata().map_err(|_| Cause::CopyUnknown)?;
    let own = meta.uid() == rustix::process::geteuid().as_raw();
    if !meta.is_file() || !own || meta.len() > MAX_RECORD {
        return Err(Cause::CopyUnknown);
    }
    let mut text = Vec::new();
    file.take(MAX_RECORD)
        .read_to_end(&mut text)
        .map_err(|_| Cause::CopyUnknown)?;
    let record: Record = serde_json::from_slice(&text).map_err(|_| Cause::CopyUnknown)?;
    (record.copy == copy && record.holds_form())
        .then_some(record)
        .ok_or(Cause::CopyUnknown)
}

/// Deletes the record of `copy`, and only the record: no file of the found model (Section 2.3).
/// A record that is not there is no error.
///
/// # Errors
/// `limit_disk` when the state folder cannot be written.
pub fn forget(state: &Path, copy: Uuid) -> Result<(), Cause> {
    let folder = folder(state)?;
    match rustix::fs::unlinkat(&folder, name_of(copy).as_str(), AtFlags::empty()) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(_) => Err(Cause::LimitDisk),
    }
}
