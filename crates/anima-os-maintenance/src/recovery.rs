//! Admission opens only after this boot has reconciled durable runtime ownership.
use crate::{Admission, Error};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
use uuid::Uuid;

const READY: &str = "/run/anima-os/reconciled-boot";
/// # Errors
/// The kernel boot identity cannot be read or parsed.
pub fn boot_id() -> Result<Uuid, Error> {
    let value = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    Uuid::parse_str(value.trim()).map_err(|_| Error::Refused("The boot identity is invalid."))
}

/// # Errors
/// Returns inaccessible or malformed recovery state instead of assuming readiness.
pub fn ready() -> Result<bool, Error> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(READY)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(Error::Refused("Recovery state is invalid."));
    }
    let mut value = String::new();
    file.take(128).read_to_string(&mut value)?;
    Ok(value == boot_id()?.to_string())
}

/// # Errors
/// Refuses new work until the boot reconciliation owner has completed its pass.
pub fn require_ready() -> Result<(), Error> {
    if !ready()? {
        return Err(Error::Refused(
            "The node is reconciling previous work. Queue work or choose another node.",
        ));
    }
    Ok(())
}

/// Called only by the root recovery owner after all runtime observations succeed.
/// # Errors
/// Returns failure when the boot marker cannot be durably published.
pub fn complete(_guard: &Admission) -> Result<(), Error> {
    let pending = Path::new("/run/anima-os/reconciled-boot.pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(pending)?;
    file.write_all(boot_id()?.to_string().as_bytes())?;
    file.sync_all()?;
    fs::rename(pending, READY)?;
    fs::File::open("/run/anima-os")?.sync_all()?;
    Ok(())
}
