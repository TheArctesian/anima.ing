//! Pin an ordinary source object in a private verification directory without following links.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read as _};

use rustix::fs::{AtFlags, fstat, linkat, statat};

use super::Dir;

impl Dir {
    pub(in crate::threads::git) fn link_file(&self, name: &OsStr, into: &Self) -> io::Result<()> {
        let mut original = self.file(name)?.ok_or(io::ErrorKind::NotFound)?;
        let held = fstat(&original)?;
        match self.hard_link(name, into) {
            Ok(()) => into.same_entry(name, &held)?,
            Err(rustix::io::Errno::XDEV) => into.copy_file(name, &mut original)?,
            Err(error) => return Err(error.into()),
        }
        self.same_entry(name, &held)?;
        original.sync_all()
    }

    fn same_entry(&self, name: &OsStr, held: &rustix::fs::Stat) -> io::Result<()> {
        let entry = statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW)?;
        if (entry.st_dev, entry.st_ino) == (held.st_dev, held.st_ino) {
            Ok(())
        } else {
            Err(io::Error::other("an object changed while pinning its file"))
        }
    }

    fn copy_file(&self, name: &OsStr, original: &mut File) -> io::Result<()> {
        let size = original.metadata()?.len();
        let mut capped = original.take(size.saturating_add(1));
        let mut copy = self.new_file(name, false)?;
        if io::copy(&mut capped, &mut copy)? == size {
            Ok(())
        } else {
            Err(io::Error::other(
                "an object changed size while copying its file",
            ))
        }
    }
}

impl Dir {
    fn hard_link(&self, name: &OsStr, into: &Self) -> Result<(), rustix::io::Errno> {
        #[cfg(test)]
        if FORCE_EXDEV.get() {
            return Err(rustix::io::Errno::XDEV);
        }
        linkat(&self.0, name, &into.0, name, AtFlags::empty())
    }
}

#[cfg(test)]
thread_local! {
    static FORCE_EXDEV: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(in crate::threads::git) fn force_exdev() -> impl Drop {
    struct Reset(std::marker::PhantomData<std::rc::Rc<()>>);

    impl Drop for Reset {
        fn drop(&mut self) {
            FORCE_EXDEV.set(false);
        }
    }

    assert!(!FORCE_EXDEV.replace(true));
    Reset(std::marker::PhantomData)
}
