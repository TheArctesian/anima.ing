//! Write private application files by replacing the directory entry after syncing.
use std::{ffi::CStr, fs::File, io, io::Write as _};

use rustix::{
    fs::{self, AtFlags, Mode, OFlags, RenameFlags},
    io::Errno,
};

use super::{Fault, Target};

const TEMPORARY_PREFIX: &str = ".write-";

pub(super) fn is_temporary(name: &CStr) -> bool {
    name.to_str()
        .ok()
        .and_then(|name| name.strip_prefix(TEMPORARY_PREFIX))
        .is_some_and(|suffix| crate::forms::is_lower_hex(suffix, 32))
}

pub(super) fn replace(target: &Target<'_>, bytes: &[u8]) -> Result<(), Fault> {
    populate(target, |file| file.write_all(bytes))
}

pub(super) fn create(target: &Target<'_>, bytes: &[u8]) -> Result<(), Fault> {
    publish(target, |file| file.write_all(bytes), RenameFlags::NOREPLACE)
}

pub(super) fn populate(
    target: &Target<'_>,
    contents: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), Fault> {
    publish(target, contents, RenameFlags::empty())
}

fn publish(
    target: &Target<'_>,
    contents: impl FnOnce(&mut File) -> io::Result<()>,
    rename_flags: RenameFlags,
) -> Result<(), Fault> {
    let temporary = temporary_name()?;
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    let fd = fs::openat(
        &target.dir,
        temporary.as_str(),
        flags,
        Mode::RUSR | Mode::WUSR,
    )?;
    let mut file = File::from(fd);
    let copied = contents(&mut file)
        .and_then(|()| file.sync_all())
        .map_err(Fault::from)
        .and_then(|()| {
            if rename_flags.is_empty() {
                fs::renameat(&target.dir, temporary.as_str(), &target.dir, target.name)?;
            } else {
                fs::renameat_with(
                    &target.dir,
                    temporary.as_str(),
                    &target.dir,
                    target.name,
                    rename_flags,
                )?;
            }
            Ok(())
        });
    if let Err(error) = copied {
        match fs::unlinkat(&target.dir, temporary.as_str(), AtFlags::empty()) {
            Ok(()) | Err(Errno::NOENT) => return Err(error),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(fs::fsync(&target.dir)?)
}

fn temporary_name() -> Result<String, Fault> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Fault::Storage(std::io::ErrorKind::Other))?;
    Ok(format!(
        "{TEMPORARY_PREFIX}{:032x}",
        u128::from_ne_bytes(bytes)
    ))
}
