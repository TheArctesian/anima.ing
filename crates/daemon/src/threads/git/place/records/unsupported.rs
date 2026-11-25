use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;

#[derive(Debug)]
pub(crate) struct Directory;

fn unsupported<T>() -> io::Result<T> {
    Err(io::ErrorKind::Unsupported.into())
}

impl Directory {
    pub(crate) fn open(_: &Path, _: &Path, _: bool) -> io::Result<Self> {
        unsupported()
    }

    pub(super) fn touch(&self, _: std::fs::FileTimes) -> io::Result<()> {
        unsupported()
    }

    pub(crate) fn names(&self) -> io::Result<Vec<OsString>> {
        unsupported()
    }

    pub(crate) fn read(&self, _: &OsStr) -> io::Result<Option<Vec<u8>>> {
        unsupported()
    }

    pub(crate) fn write(&self, _: &OsStr, _: &[u8]) -> io::Result<()> {
        unsupported()
    }

    pub(crate) fn remove(&self, _: &OsStr) -> io::Result<()> {
        unsupported()
    }

    pub(crate) fn remove_tree(&self, _: &OsStr) -> io::Result<()> {
        unsupported()
    }
}
