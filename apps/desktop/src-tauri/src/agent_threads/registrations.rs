//! Pin each watched inode through no-follow, nonblocking opens before native registration.
use super::folders;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeMap,
    fs::File,
    io,
    os::fd::AsRawFd as _,
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
            let file = folders::open(home, &path, identity)?;
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

#[cfg(test)]
mod tests;
