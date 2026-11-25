//! Pin an ordinary source object in a private verification directory without following links.

use std::ffi::OsStr;
use std::io;

use rustix::fs::{AtFlags, fstat, linkat, statat};

use super::Dir;

impl Dir {
    pub(in crate::threads::git) fn link_file(&self, name: &OsStr, into: &Self) -> io::Result<()> {
        let original = self.file(name)?.ok_or(io::ErrorKind::NotFound)?;
        let held = fstat(&original)?;
        linkat(&self.0, name, &into.0, name, AtFlags::empty())?;
        for dir in [self, into] {
            let entry = statat(&dir.0, name, AtFlags::SYMLINK_NOFOLLOW)?;
            if (entry.st_dev, entry.st_ino) != (held.st_dev, held.st_ino) {
                return Err(io::Error::other("an object changed while pinning its file"));
            }
        }
        original.sync_all()
    }
}
