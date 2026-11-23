//! Files in the hub's folder opened without trust, so a link or pipe in their place cannot block or flood a read.

#[cfg(unix)]
use std::fs::{File, OpenOptions, Permissions};
use std::io::{self, Read as _};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

#[cfg(unix)]
use nix::fcntl::OFlag;

#[cfg(unix)]
fn refuse_what_is_not_a_file(file: &File) -> io::Result<()> {
    if file.metadata()?.is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ))
    }
}

#[cfg(unix)]
const NO_LINK_NO_WAIT: i32 = OFlag::O_NOFOLLOW.bits() | OFlag::O_NONBLOCK.bits();

#[cfg(unix)]
pub(crate) fn appended(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(NO_LINK_NO_WAIT)
        .open(path)?;
    refuse_what_is_not_a_file(&file)?;
    file.set_permissions(Permissions::from_mode(0o600))?;
    Ok(file)
}

pub(crate) fn read_text(path: &Path, limit: u64) -> io::Result<String> {
    #[cfg(unix)]
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(NO_LINK_NO_WAIT)
        .open(path)?;
    #[cfg(unix)]
    refuse_what_is_not_a_file(&file)?;
    #[cfg(windows)]
    let file = anima_windows::files::read(path, true)?;
    let mut text = String::new();
    file.take(limit + 1).read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "too long"));
    }
    Ok(text)
}

#[cfg(windows)]
pub(crate) use anima_database::owner_only::appended_file as appended;
