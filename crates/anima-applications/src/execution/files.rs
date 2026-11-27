//! File operations addressed through an exclusively owned application-store descriptor.
mod imports;
mod removal;
mod write;

use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read as _},
    os::fd::OwnedFd,
    path::{Component, Path},
};

use rustix::{
    fs::{self, AtFlags, FileType, FlockOperation, Mode, OFlags, ResolveFlags, Stat},
    io::Errno,
    process::geteuid,
};

use super::Fault;

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_XDEV);

pub(crate) struct Store<'a> {
    root: &'a Path,
    fd: OwnedFd,
}

struct Target<'a> {
    dir: OwnedFd,
    name: &'a OsStr,
}

impl<'a> Store<'a> {
    pub(crate) fn open(root: &'a Path) -> Result<Self, Fault> {
        let store = Self::observe(root)?;
        fs::flock(&store.fd, FlockOperation::NonBlockingLockExclusive)?;
        Ok(store)
    }

    pub(crate) fn observe(root: &'a Path) -> Result<Self, Fault> {
        if !root.is_absolute() || root.parent().is_none() || root.canonicalize()? != root {
            return Err(Fault::UnsafePlan);
        }
        let fd = fs::open(root, DIRECTORY, Mode::empty())?;
        private(&fd)?;
        Ok(Self { root, fd })
    }

    fn target<'p>(&self, path: &'p str) -> Result<Target<'p>, Fault> {
        let relative = Path::new(path)
            .strip_prefix(self.root)
            .map_err(|_| Fault::UnsafePlan)?;
        let mut parts = relative.components();
        let Some(Component::Normal(name)) = parts.next_back() else {
            return Err(Fault::UnsafePlan);
        };
        let mut dir = directory(&self.fd, OsStr::new("."))?;
        for part in parts {
            let Component::Normal(part) = part else {
                return Err(Fault::UnsafePlan);
            };
            dir = directory(&dir, part)?;
            private(&dir)?;
        }
        Ok(Target { dir, name })
    }

    pub(crate) fn folder(&self, path: &str) -> Result<(), Fault> {
        let target = self.target(path)?;
        mkdir(&target)?;
        private(&directory(&target.dir, target.name)?)?;
        Ok(fs::fsync(&target.dir)?)
    }

    pub(super) fn bind_folder(&self, path: &str) -> Result<(), Fault> {
        let relative = Path::new(path)
            .strip_prefix(self.root)
            .map_err(|_| Fault::UnsafePlan)?;
        let mut parts = relative.iter();
        let application = parts.next().and_then(OsStr::to_str);
        if !application.is_some_and(crate::forms::is_uuid)
            || parts.next() != Some(OsStr::new("binds"))
            || !parts
                .next()
                .and_then(OsStr::to_str)
                .is_some_and(compose_rules::forms::is_name)
            || parts.next().is_some()
        {
            return Err(Fault::UnsafePlan);
        }
        let target = self.target(path)?;
        mkdir(&target)?;
        directory(&target.dir, target.name)?;
        Ok(fs::fsync(&target.dir)?)
    }

    pub(super) fn real_folder(&self, path: &str, below: &str) -> Result<(), Fault> {
        if Path::new(path).parent() != Some(Path::new(below)) {
            return Err(Fault::UnsafePlan);
        }
        let target = self.target(path)?;
        private(&directory(&target.dir, target.name)?)
    }

    pub(crate) fn existing_bind(&self, path: &str) -> Result<(), Fault> {
        let target = self.target(path)?;
        directory(&target.dir, target.name)?;
        Ok(())
    }

    pub(crate) fn absent(&self, path: &str) -> Result<(), Fault> {
        let target = self.target(path)?;
        if stat(&target)?.is_some() {
            return Err(Fault::UnsafePlan);
        }
        Ok(())
    }

    pub(crate) fn write(&self, path: &str, bytes: &[u8]) -> Result<(), Fault> {
        write::replace(&self.target(path)?, bytes)
    }

    pub(crate) fn create(&self, path: &str, bytes: &[u8]) -> Result<(), Fault> {
        write::create(&self.target(path)?, bytes)
    }

    pub(crate) fn sync_record(&self, path: &str) -> Result<(), Fault> {
        self.open_private_file(path)?.sync_all()?;
        Ok(fs::fsync(&self.target(path)?.dir)?)
    }

    pub(crate) fn read_small_file(&self, path: &str, limit: u64) -> Result<Vec<u8>, Fault> {
        let mut bytes = Vec::new();
        self.open_private_file(path)?
            .take(limit + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(Fault::Storage(io::ErrorKind::InvalidData));
        }
        Ok(bytes)
    }

    pub(super) fn copy_file(&self, from: &str, to: &str) -> Result<(), Fault> {
        let mut source = self.open_private_file(from)?;
        write::populate(&self.target(to)?, |target| {
            io::copy(&mut source, target).map(|_| ())
        })
    }

    fn open_private_file(&self, path: &str) -> Result<File, Fault> {
        let target = self.target(path)?;
        let file = fs::openat2(
            &target.dir,
            target.name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            RESOLVE,
        )?;
        private_file(&fs::fstat(&file)?)?;
        Ok(File::from(file))
    }

    pub(crate) fn has_private_file(&self, path: &str) -> Result<bool, Fault> {
        let target = match self.target(path) {
            Err(Fault::Storage(io::ErrorKind::NotFound)) => return Ok(false),
            result => result?,
        };
        let Some(metadata) = stat(&target)? else {
            return Ok(false);
        };
        private_file(&metadata)?;
        Ok(true)
    }

    pub(super) fn move_file(&self, from: &str, to: &str) -> Result<(), Fault> {
        let source = match self.target(from) {
            Ok(target) => target,
            Err(Fault::Storage(std::io::ErrorKind::NotFound)) => return Ok(()),
            Err(error) => return Err(error),
        };
        let Some(metadata) = stat(&source)? else {
            return Ok(());
        };
        private_file(&metadata)?;
        let target = self.target(to)?;
        fs::renameat(&source.dir, source.name, &target.dir, target.name)?;
        fs::fsync(&target.dir)?;
        Ok(fs::fsync(&source.dir)?)
    }

    pub(super) fn delete_file(&self, path: &str) -> Result<(), Fault> {
        self.remove(path, removal::file)
    }

    pub(super) fn delete_folder(&self, path: &str) -> Result<(), Fault> {
        self.remove(path, removal::folder)
    }

    fn remove(
        &self,
        path: &str,
        delete: fn(&Target<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        match self.target(path) {
            Ok(target) => delete(&target),
            Err(Fault::Storage(std::io::ErrorKind::NotFound)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn mkdir(target: &Target<'_>) -> Result<(), Fault> {
    match fs::mkdirat(&target.dir, target.name, Mode::RWXU) {
        Ok(()) | Err(Errno::EXIST) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn directory(dir: &OwnedFd, name: &OsStr) -> Result<OwnedFd, Fault> {
    Ok(fs::openat2(dir, name, DIRECTORY, Mode::empty(), RESOLVE)?)
}

fn private(dir: &OwnedFd) -> Result<(), Fault> {
    let metadata = fs::fstat(dir)?;
    if metadata.st_uid != geteuid().as_raw() || metadata.st_mode & 0o077 != 0 {
        return Err(Fault::UnsafePlan);
    }
    Ok(())
}

fn private_file(metadata: &Stat) -> Result<(), Fault> {
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile
        || metadata.st_uid != geteuid().as_raw()
        || metadata.st_mode & 0o077 != 0
        || metadata.st_nlink != 1
    {
        return Err(Fault::UnsafePlan);
    }
    Ok(())
}

fn stat(target: &Target<'_>) -> Result<Option<Stat>, Fault> {
    match fs::statat(&target.dir, target.name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
