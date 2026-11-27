use super::{Error, Lease, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const ROOT: &str = "/run/anima-previews";
fn path(id: Uuid) -> Result<PathBuf> {
    if id.is_nil() {
        return Err(Error::Refused("A preview identity is required."));
    }
    Ok(Path::new(ROOT).join(format!("{id}.json")))
}

pub fn lock(id: Uuid) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path(id)?.with_extension("lock"))?;
    file.try_lock()
        .map_err(|_| Error::Refused("Another operation owns this preview."))?;
    Ok(file)
}
pub fn read(id: Uuid) -> Result<Option<Lease>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path(id)?)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(Error::Refused("Preview state is invalid."));
    }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused("Preview state exceeds its limit."));
    }
    let lease: Lease = serde_json::from_slice(&bytes)?;
    if lease.schema != 1 || lease.binding.endpoint != id || lease.revision != 1 {
        return Err(Error::Refused("Preview state is incompatible."));
    }
    Ok(Some(lease))
}
pub fn save(lease: &Lease) -> Result<()> {
    let target = path(lease.binding.endpoint)?;
    let pending = target.with_extension("pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(lease)?)?;
    file.sync_all()?;
    fs::rename(pending, target)?;
    fs::File::open(ROOT)?.sync_all()?;
    Ok(())
}
