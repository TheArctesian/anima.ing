//! Publishes one synced file from a directory used exclusively by this copy.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Seek as _};
use std::os::fd::AsFd as _;

use rustix::fs::{AtFlags, Mode, OFlags, fstat, mkdirat, openat, renameat, statat};

use super::Dir;

const CONTENT: &str = "content";

impl Dir {
    pub(in crate::threads::git) fn publish(&self, name: &OsStr, source: File) -> io::Result<()> {
        self.publish_checked(name, source, |_| Ok(()))
    }

    pub(in crate::threads::git) fn publish_checked(
        &self,
        name: &OsStr,
        mut source: File,
        check: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<()> {
        let temporary = format!(".anima-copy-{}", uuid::Uuid::now_v7().as_simple());
        let temporary = temporary.as_ref();
        mkdirat(&self.0, temporary, Mode::RWXU)?;
        let result = (|| {
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            let stage = Dir(openat(&self.0, temporary, flags, Mode::empty())?);
            let copied = stage.publish_from((self, name), &mut source, check);
            let _ = stage.remove(CONTENT.as_ref(), false);
            copied
        })();
        let cleared = self.remove(temporary, true);
        result?;
        cleared?;
        self.sync()
    }

    fn publish_from(
        &self,
        (into, name): (&Dir, &OsStr),
        source: &mut File,
        check: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<()> {
        // Inherited macOS ACLs can otherwise allow others into a mode-0700 directory.
        #[cfg(target_os = "macos")]
        if !calcifer_macos_acl::read_acl(self.0.as_fd())?.is_empty() {
            calcifer_macos_acl::clear_acl(self.0.as_fd())?;
        }
        let flags =
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mode = Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH;
        let mut written = File::from(openat(&self.0, CONTENT, flags, mode)?);
        io::copy(source, &mut written)?;
        written.sync_all()?;
        written.rewind()?;
        check(&written)?;
        let owned = fstat(written.as_fd())?;
        let named = statat(&self.0, CONTENT, AtFlags::SYMLINK_NOFOLLOW)?;
        if (owned.st_dev, owned.st_ino) != (named.st_dev, named.st_ino) {
            return Err(io::Error::other("the copy's temporary file changed"));
        }
        renameat(&self.0, CONTENT, &into.0, name)?;
        self.sync()
    }
}
