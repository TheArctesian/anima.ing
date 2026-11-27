//! Which folder a path names now (P12): the runner writes it down when it makes a worktree and
//! looks again before each run, so that a folder put in its place gets no run.
//!
//! I/O: reads the metadata of one path.

use std::path::Path;

/// The device and the number of a folder on its file system: a different folder at the same
/// path, or a link to one, does not have them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FolderId {
    device: u64,
    number: u64,
}

impl FolderId {
    /// The ID of the folder at `path`; `None` for a link, a file or nothing.
    #[cfg(unix)]
    #[must_use]
    pub fn of(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        let about = path
            .symlink_metadata()
            .ok()
            .filter(std::fs::Metadata::is_dir)?;
        Some(Self {
            device: about.dev(),
            number: about.ino(),
        })
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(in crate::threads::git) fn from_directory(
        fd: std::os::fd::OwnedFd,
    ) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt as _;
        let about = std::fs::File::from(fd).metadata()?;
        Ok(Self {
            device: about.dev(),
            number: about.ino(),
        })
    }

    /// Windows gives no such number to safe code; the check of each call stays.
    #[cfg(not(unix))]
    #[must_use]
    pub const fn of(_path: &Path) -> Option<Self> {
        None
    }
}
