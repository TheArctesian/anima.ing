use super::{Error, MAX_BYTES, Policy, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

fn owned(metadata: &fs::Metadata) -> Result<()> {
    if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(Error::Refused(
            "Work policy storage must be writable only by root.",
        ));
    }
    Ok(())
}

pub(super) fn lock(root: &Path) -> Result<fs::File> {
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() {
        return Err(Error::Refused("Work policy storage must be a directory."));
    }
    owned(&metadata)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(root.join("lock"))?;
    regular(&file)?;
    file.try_lock()
        .map_err(|_| Error::Refused("Another operation owns the work policy."))?;
    Ok(file)
}

fn regular(file: &fs::File) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(Error::Refused("Work policy state must be a regular file."));
    }
    owned(&metadata)
}

pub(super) fn read(root: &Path) -> Result<Policy> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(root.join("policy.json"))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Policy::default()),
        Err(error) => return Err(error.into()),
    };
    regular(&file)?;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES * 6 + 1024 + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES * 6 + 1024 {
        return Err(Error::Refused("Stored work policy exceeded its limit."));
    }
    let policy: Policy = serde_json::from_slice(&bytes)?;
    policy.validate()?;
    Ok(policy)
}

pub(super) fn save(root: &Path, policy: &Policy) -> Result<()> {
    let pending = root.join("policy.pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&pending)?;
    regular(&file)?;
    file.set_len(0)?;
    file.write_all(&serde_json::to_vec(policy)?)?;
    file.sync_all()?;
    fs::rename(pending, root.join("policy.json"))?;
    fs::File::open(root)?.sync_all()?;
    Ok(())
}
