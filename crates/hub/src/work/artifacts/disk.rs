use super::{Error, MAX_RETAINED, Offer, Receipt};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

// The lock and checkpoint folder do not count against the retained-object limit.
const ROOT_ENTRIES: usize = MAX_RETAINED + 2;

pub(super) fn private(path: &Path, directory: bool) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || (directory && !metadata.is_dir())
        || (!directory && (!metadata.is_file() || metadata.nlink() != 1))
    {
        return Err(Error::Private);
    }
    Ok(())
}

pub(super) fn open(root: &Path) -> Result<File, Error> {
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => sync(root.parent().ok_or(Error::Private)?)?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.into()),
    }
    private(root, true)?;
    let path = root.join(".lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(&path)?;
    private(&path, false)?;
    lock.try_lock().map_err(|_| Error::Busy)?;
    Ok(lock)
}

pub(super) fn contents(path: &Path, permitted: &[&str]) -> Result<(), Error> {
    private(path, true)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !permitted.iter().any(|name| entry.file_name() == *name) {
            return Err(Error::Private);
        }
        private(&entry.path(), false)?;
    }
    Ok(())
}

/// Only uncommitted directories in this store's closed layout are disposable.
pub(super) fn recover(root: &Path) -> Result<(), Error> {
    let mut pending = Vec::new();
    for (count, entry) in fs::read_dir(root)?.enumerate() {
        if count >= ROOT_ENTRIES {
            return Err(Error::Capacity);
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(Error::Private)?;
        if name == super::checkpoints::DIRECTORY {
            super::checkpoints::recover(&entry.path())?;
        } else if let Some(suffix) = name.strip_prefix(".incoming-") {
            if suffix.len() != 16 || !suffix.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return Err(Error::Private);
            }
            contents(&entry.path(), &["archive.oci", "receipt.json"])?;
            pending.push(entry.path());
        } else if name != ".lock" {
            receipt(&entry.path())?;
        }
    }
    for path in pending {
        fs::remove_dir_all(path)?;
    }
    sync(root)
}

pub(super) fn receipt(path: &Path) -> Result<Receipt, Error> {
    contents(path, &["archive.oci", "receipt.json"])?;
    let mut encoded = Vec::new();
    File::open(path.join("receipt.json"))?
        .take(4097)
        .read_to_end(&mut encoded)?;
    if encoded.len() > 4096 {
        return Err(Error::Invalid);
    }
    let receipt: Receipt = serde_json::from_slice(&encoded).map_err(|_| Error::Invalid)?;
    if !receipt.valid()
        || path.file_name().and_then(|s| s.to_str())
            != Some(super::model::directory(&receipt.artifact).as_str())
        || fs::metadata(path.join("archive.oci"))?.len() != receipt.artifact.bytes
    {
        return Err(Error::Invalid);
    }
    Ok(receipt)
}

pub(super) fn inventory(root: &Path) -> Result<(u64, usize), Error> {
    let mut used = 0_u64;
    let mut entries = 0;
    for (count, entry) in fs::read_dir(root)?.enumerate() {
        if count >= ROOT_ENTRIES {
            return Err(Error::Capacity);
        }
        let entry = entry?;
        if entry.file_name() == ".lock" {
            continue;
        }
        let (bytes, count) = if entry.file_name() == super::checkpoints::DIRECTORY {
            super::checkpoints::inventory(&entry.path())?
        } else {
            (receipt(&entry.path())?.artifact.bytes, 1)
        };
        used = used.checked_add(bytes).ok_or(Error::Capacity)?;
        entries += count;
        if entries > MAX_RETAINED {
            return Err(Error::Capacity);
        }
    }
    Ok((used, entries))
}

pub(super) fn capacity(root: &Path, reserve: u64, bytes: u64) -> Result<(), Error> {
    let stat = rustix::fs::statvfs(root).map_err(std::io::Error::from)?;
    let free = stat.f_bavail.saturating_mul(stat.f_frsize);
    if reserve
        .checked_add(bytes)
        .is_none_or(|needed| needed > free)
    {
        return Err(Error::Capacity);
    }
    Ok(())
}

pub(super) fn verified_file(path: &Path, offer: &Offer) -> Result<File, Error> {
    verified_bytes(path, offer.bytes, &offer.archive_digest)
}

pub(super) fn verified_bytes(path: &Path, length: u64, digest: &str) -> Result<File, Error> {
    let mut source = archive(path, length)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 65536];
    let mut bytes = 0_u64;
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > length {
            return Err(Error::Invalid);
        }
        hasher.update(&buffer[..count]);
    }
    if bytes != length || super::model::encoded_digest(&hasher.finalize()) != digest {
        return Err(Error::Invalid);
    }
    source.rewind()?;
    Ok(source)
}

pub(super) fn image(path: &Path, offer: &Offer) -> Result<oci_artifact::Image, Error> {
    let mut file = archive(path, offer.bytes)?;
    oci_artifact::verify(&mut file, claim(offer))
        .map(|verified| verified.image)
        .map_err(image_error)
}

pub(super) fn publication(
    path: &Path,
    offer: &Offer,
) -> Result<(File, oci_artifact::Publication), Error> {
    let mut file = archive(path, offer.bytes)?;
    let plan = oci_artifact::publication(&mut file, claim(offer)).map_err(image_error)?;
    Ok((file, plan))
}

fn claim(offer: &Offer) -> oci_artifact::Claim<'_> {
    oci_artifact::Claim {
        bytes: offer.bytes,
        archive_digest: &offer.archive_digest,
        manifest_digest: &offer.manifest_digest,
    }
}

fn image_error(error: oci_artifact::Error) -> Error {
    match error {
        oci_artifact::Error::Io(error) => Error::Io(error.kind()),
        _ => Error::Image,
    }
}

pub(super) fn archive(path: &Path, bytes: u64) -> Result<File, Error> {
    private(path, false)?;
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let source = OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits().cast_signed())
        .open(path)?;
    let metadata = source.metadata()?;
    if !metadata.is_file() || metadata.len() != bytes {
        return Err(Error::Invalid);
    }
    Ok(source)
}

pub(super) fn write_receipt(path: &Path, receipt: &Receipt) -> Result<(), Error> {
    write_metadata(path, receipt)
}

pub(super) fn write_metadata(path: &Path, receipt: &impl serde::Serialize) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.join("receipt.json"))?;
    let encoded = serde_json::to_vec(receipt).map_err(|_| Error::Invalid)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    sync(path)
}

pub(super) fn sync(path: &Path) -> Result<(), Error> {
    File::open(path)?.sync_all().map_err(Into::into)
}
