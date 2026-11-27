//! Install a prepared application directory by moving one locked directory entry.
mod tree;

use std::{ffi::OsStr, os::fd::OwnedFd};

use rustix::fs::{self, AtFlags, Dir, FlockOperation, RenameFlags};

use super::{Fault, Store, directory, private, private_file, write};

pub(crate) struct ImportFolder(OwnedFd);

impl Store<'_> {
    pub(crate) fn lock_import(&self, path: &str) -> Result<ImportFolder, Fault> {
        let target = self.target(path)?;
        let folder = directory(&target.dir, target.name)?;
        private(&folder)?;
        fs::flock(&folder, FlockOperation::NonBlockingLockExclusive)?;
        clear_interrupted_writes(&folder)?;
        Ok(ImportFolder(folder))
    }

    pub(crate) fn inspect_import(
        &self,
        folder: &ImportFolder,
        record: &str,
    ) -> Result<Vec<String>, Fault> {
        let binds = tree::verify(&folder.0)?;
        self.open_private_file(record)?.sync_all()?;
        fs::fsync(&folder.0)?;
        Ok(binds)
    }

    pub(crate) fn install_import(
        &self,
        paths: (&str, &str),
        folder: &ImportFolder,
    ) -> Result<(), Fault> {
        let source = self.target(paths.0)?;
        let destination = self.target(paths.1)?;
        let named = directory(&source.dir, source.name)?;
        let (expected, present) = (fs::fstat(&folder.0)?, fs::fstat(named)?);
        if expected.st_ino != present.st_ino || expected.st_dev != present.st_dev {
            return Err(Fault::UnsafePlan);
        }
        fs::renameat_with(
            &source.dir,
            source.name,
            &destination.dir,
            destination.name,
            RenameFlags::NOREPLACE,
        )?;
        self.sync_import(paths)
    }

    pub(crate) fn sync_import(&self, (source, destination): (&str, &str)) -> Result<(), Fault> {
        fs::fsync(&self.target(destination)?.dir)?;
        fs::fsync(&self.target(source)?.dir)?;
        Ok(())
    }
}

// Capture writes a manifest inside the otherwise closed import folder.
fn clear_interrupted_writes(folder: &OwnedFd) -> Result<(), Fault> {
    let mut cleared = false;
    for entry in Dir::read_from(folder)? {
        let entry = entry?;
        let name = entry.file_name();
        if write::is_temporary(name) {
            private_file(&fs::statat(folder, name, AtFlags::SYMLINK_NOFOLLOW)?)?;
            fs::unlinkat(folder, name, AtFlags::empty())?;
            cleared = true;
        }
    }
    if cleared {
        fs::fsync(folder)?;
    }
    Ok(())
}

fn binds(parent: &OwnedFd) -> Result<OwnedFd, Fault> {
    let folder = directory(parent, OsStr::new("binds"))?;
    private(&folder)?;
    Ok(folder)
}
