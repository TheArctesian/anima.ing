#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use super::super::access::Groups;
use super::super::files::Root;

#[derive(Clone, Debug)]
pub struct Here {
    pub home: PathBuf,
    pub path: Vec<PathBuf>,
    pub user: u32,
    pub groups: Groups,
    pub macos: bool,
    /// Windows roaming application settings, including redirected user profiles.
    #[cfg(windows)]
    pub roaming: Option<PathBuf>,
}

impl Here {
    /// This computer for the user that runs the process, or `None` when that user is not known
    /// or does not own `home` (as under `sudo`): the files would then belong to the wrong user.
    #[must_use]
    pub fn for_this_user(home: PathBuf, path: Vec<PathBuf>) -> Option<Self> {
        // The user of this process is the owner of a file that it makes.
        #[cfg(unix)]
        let user = tempfile::tempfile()
            .and_then(|file| file.metadata())
            .ok()?
            .uid();
        #[cfg(unix)]
        let owned = fs::metadata(&home).is_ok_and(|metadata| metadata.uid() == user);
        #[cfg(windows)]
        let (user, owned) = (
            0,
            anima_windows::files::secure(&home, false).unwrap_or(false),
        );
        owned.then_some(Self {
            home,
            path,
            user,
            groups: Groups::system(),
            macos: cfg!(target_os = "macos"),
            #[cfg(windows)]
            roaming: std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .filter(|path| {
                    path.is_absolute()
                        && path
                            .ancestors()
                            .all(|part| anima_windows::files::secure(part, false).unwrap_or(false))
                }),
        })
    }

    pub(super) fn root(&self, path: &Path) -> Root<'_> {
        let home = self.home.as_path();
        #[cfg(windows)]
        let home = self
            .roaming
            .as_deref()
            .filter(|root| path.starts_with(root))
            .unwrap_or(home);
        #[cfg(unix)]
        let _ = path;
        Root {
            home,
            user: self.user,
            groups: &self.groups,
        }
    }
}
