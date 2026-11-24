//! Pin each watched inode through no-follow, nonblocking opens before native registration.
use super::folders;
use nix::{
    fcntl::{OFlag, openat},
    sys::stat::Mode,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io,
    os::{
        fd::AsRawFd as _,
        unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    },
    path::{Path, PathBuf},
};

type Identity = (u64, u64);

struct Registered {
    identity: Identity,
    native: PathBuf,
    _file: File,
}

#[derive(Default)]
pub(super) struct Registrations(BTreeMap<PathBuf, Registered>);

impl Registrations {
    pub(super) fn reconcile(
        &mut self,
        watcher: &mut RecommendedWatcher,
        home: &Path,
    ) -> io::Result<()> {
        let desired = folders::discover(home)?;
        let retired: Vec<_> = self
            .0
            .iter()
            .filter(|(path, registration)| desired.get(*path) != Some(&registration.identity))
            .map(|(path, _)| path.clone())
            .collect();
        for path in retired {
            if let Some(registration) = self.0.remove(&path) {
                match watcher.unwatch(&registration.native) {
                    Ok(()) => (),
                    Err(error) if matches!(error.kind, notify::ErrorKind::WatchNotFound) => (),
                    Err(_) => return Err(io::Error::other("session watch removal failed")),
                }
            }
        }
        for (path, identity) in desired {
            if self.0.contains_key(&path) {
                continue;
            }
            let file = open(home, &path, identity)?;
            #[cfg(target_os = "macos")]
            let native = PathBuf::from(format!("/dev/fd/{}", file.as_raw_fd()));
            #[cfg(not(target_os = "macos"))]
            let native = PathBuf::from(format!("/proc/self/fd/{}/.", file.as_raw_fd()));
            watcher
                .watch(&native, RecursiveMode::NonRecursive)
                .map_err(|_| io::Error::other("session watch registration failed"))?;
            self.0.insert(
                path,
                Registered {
                    identity,
                    native,
                    _file: file,
                },
            );
        }
        Ok(())
    }
}

/// No path component can redirect the watch outside the supplied home. No FIFO can block it.
fn open(home: &Path, path: &Path, identity: Identity) -> io::Result<File> {
    let flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((flags | OFlag::O_DIRECTORY).bits())
        .open(home)?;
    for component in path
        .strip_prefix(home)
        .map_err(io::Error::other)?
        .components()
    {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(io::Error::other("invalid watch path"));
        }
        file = File::from(openat(&file, component.as_os_str(), flags, Mode::empty())?);
    }
    let metadata = file.metadata()?;
    if (metadata.dev(), metadata.ino()) != identity
        || !(metadata.is_dir()
            || (metadata.is_file()
                && metadata.nlink() == 1
                && metadata.uid() == nix::unistd::getuid().as_raw()))
    {
        return Err(io::Error::other("session watch inode changed"));
    }
    Ok(file)
}

#[cfg(test)]
mod tests;
