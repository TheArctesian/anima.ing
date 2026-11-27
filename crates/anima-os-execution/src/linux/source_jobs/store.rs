use super::{Error, Record, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const ROOT: &str = "/persist/anima-source-jobs";
const LIMIT: usize = 65536;

fn owned(metadata: &fs::Metadata) -> Result<()> {
    if metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err(Error::Refused(
            "Source-job storage must be private to root.",
        ));
    }
    Ok(())
}

fn regular(file: &fs::File) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(Error::Refused("Source-job state must be a regular file."));
    }
    owned(&metadata)
}

pub(super) fn lock() -> Result<fs::File> {
    match fs::DirBuilder::new().mode(0o700).create(ROOT) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(ROOT)?;
    if !metadata.is_dir() {
        return Err(Error::Refused("Source-job storage must be a directory."));
    }
    owned(&metadata)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(Path::new(ROOT).join("lock"))?;
    regular(&file)?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => Error::SourceJournalBusy,
        fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(file)
}

fn path(id: Uuid) -> Result<PathBuf> {
    if id.is_nil() {
        return Err(Error::Refused("Choose a valid source execution identity."));
    }
    Ok(Path::new(ROOT).join(format!("{id}.json")))
}

pub(super) fn read(id: Uuid) -> Result<Option<Record>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path(id)?)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    regular(&file)?;
    let mut bytes = Vec::new();
    file.take(LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Err(Error::Refused("Source-job state exceeded its bound."));
    }
    let record: Record = serde_json::from_slice(&bytes)?;
    if record.schema != 1
        || record.execution != id
        || record.job.id != record.lease.grant.job
        || record.job.attempt != record.lease.grant.attempt
    {
        return Err(Error::Refused(
            "Source-job state has inconsistent ownership.",
        ));
    }
    Ok(Some(record))
}

pub(super) fn save(record: &Record) -> Result<()> {
    let bytes = serde_json::to_vec(record)?;
    if bytes.len() > LIMIT {
        return Err(Error::Refused("Source-job state exceeded its bound."));
    }
    let target = path(record.execution)?;
    let pending = target.with_extension("pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&pending)?;
    regular(&file)?;
    file.set_len(0)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(pending, target)?;
    fs::File::open(ROOT)?.sync_all()?;
    Ok(())
}

pub(super) fn forget(id: Uuid) -> Result<()> {
    let target = path(id)?;
    match fs::symlink_metadata(target.with_extension("pending")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
        Ok(_) => return Err(Error::Refused("Source-job metadata has a pending write.")),
    }
    match fs::remove_file(target) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    fs::File::open(ROOT)?.sync_all()?;
    Ok(())
}

pub(super) fn accept(device: Uuid, origin: &str, job: &daemon_core::wire::WorkJob) -> Result<()> {
    let mut count = 0;
    for entry in fs::read_dir(ROOT)? {
        let name = entry?.file_name();
        if name == "lock" {
            continue;
        }
        let path = Path::new(&name);
        let id: Uuid = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse().ok())
            .ok_or(Error::Refused(
                "Source-job registration inventory is invalid.",
            ))?;
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => {
                let existing =
                    read(id)?.ok_or(Error::Refused("Source-job registration disappeared."))?;
                if existing.device == device
                    && existing.origin == origin
                    && existing.job.id == job.id
                    && existing.job.attempt == job.attempt
                {
                    return Err(Error::Refused(
                        "This hub attempt already has a registered execution.",
                    ));
                }
            }
            Some("pending") => (),
            _ => {
                return Err(Error::Refused(
                    "Source-job registration inventory is invalid.",
                ));
            }
        }
        count += 1;
        if count >= 128 {
            return Err(Error::Refused("Source-job registration inventory is full."));
        }
    }
    Ok(())
}

pub(super) fn ids() -> Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(ROOT)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse().ok())
            .ok_or(Error::Refused(
                "Source-job registration inventory is invalid.",
            ))?;
        ids.push(id);
        if ids.len() > 128 {
            return Err(Error::Refused("Source-job inventory exceeded its bound."));
        }
    }
    Ok(ids)
}
