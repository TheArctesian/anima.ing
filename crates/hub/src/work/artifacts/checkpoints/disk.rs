use super::{DIRECTORY, Error, Import, Store, Stored, Uuid};
use crate::work::artifacts::{MAX_RETAINED, disk};
use chrono::{DateTime, Utc};
use std::{
    fs::{self, File},
    io::{self, Read},
    os::unix::fs::DirBuilderExt as _,
    path::Path,
};

const FILES: &[&str] = &["project.tar", "receipt.json"];

pub(super) fn create(root: &Path) -> Result<(), Error> {
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => disk::sync(root.parent().ok_or(Error::Private)?)?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.into()),
    }
    disk::private(root, true)
}

pub(super) fn stored(path: &Path) -> Result<Stored, Error> {
    disk::contents(path, FILES)?;
    let mut bytes = Vec::new();
    File::open(path.join("receipt.json"))?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(Error::Invalid);
    }
    let receipt: Stored = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
    if !receipt.import.fits()
        || path.file_name().and_then(|name| name.to_str())
            != Some(receipt.transfer().to_string().as_str())
        || fs::metadata(path.join("project.tar"))?.len() != receipt.import.receipt.bytes
    {
        return Err(Error::Invalid);
    }
    Ok(receipt)
}

pub(super) fn open(store: &Store, import: &Import) -> Result<File, Error> {
    let path = store
        .0
        .root
        .join(DIRECTORY)
        .join(import.receipt.source.binding.transfer.to_string());
    let held = stored(&path)?;
    if &held.import != import {
        return Err(Error::Conflict);
    }
    disk::archive(&path.join("project.tar"), import.receipt.bytes)
}

pub(in crate::work::artifacts) fn inventory(root: &Path) -> Result<(u64, usize), Error> {
    disk::private(root, true)?;
    let mut used = 0_u64;
    let mut count = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        count += 1;
        if count > MAX_RETAINED {
            return Err(Error::Capacity);
        }
        used = used
            .checked_add(stored(&entry.path())?.import.receipt.bytes)
            .ok_or(Error::Capacity)?;
    }
    Ok((used, count))
}

pub(in crate::work::artifacts) fn recover(root: &Path) -> Result<(), Error> {
    purge(root, Predicate::Incoming)
}

pub(super) fn reap(root: &Path, now: DateTime<Utc>) -> Result<(), Error> {
    if !root.try_exists()? {
        return Ok(());
    }
    purge(root, Predicate::Expired(now))
}

#[derive(Clone, Copy)]
enum Predicate {
    Incoming,
    Expired(DateTime<Utc>),
}

fn purge(root: &Path, condition: Predicate) -> Result<(), Error> {
    disk::private(root, true)?;
    let mut removed = false;
    for (index, entry) in fs::read_dir(root)?.enumerate() {
        if index > MAX_RETAINED {
            return Err(Error::Capacity);
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::Private)?;
        if disposable(&entry.path(), name, condition)? {
            fs::remove_dir_all(entry.path())?;
            removed = true;
        }
    }
    if removed {
        disk::sync(root)?;
    }
    Ok(())
}

fn disposable(path: &Path, name: &str, condition: Predicate) -> Result<bool, Error> {
    if let Some(id) = name.strip_prefix(".incoming-") {
        if id.len() != 16 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(Error::Private);
        }
        disk::contents(path, FILES)?;
        return Ok(true);
    }
    Uuid::parse_str(name).map_err(|_| Error::Private)?;
    let receipt = stored(path)?;
    Ok(matches!(condition, Predicate::Expired(now) if now >= receipt.expires_at))
}
