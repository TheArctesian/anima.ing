//! The files of a worktree, read and written with no link followed (P12, THR-050). A process
//! that an agent left behind can put a link in the place of a file or a folder at any moment.
//! `git` looks at a path and then opens it by name, so between the two it can be led out of
//! the worktree: it would read a file of the person into a snapshot, or write a file of a
//! checkpoint to a place outside. So no `git` of the runner reads the content of a file of
//! the worktree or writes one. The runner copies each file between the worktree and a folder
//! of its own, and `git` works on that folder.
//!
//! Each path is walked one part at a time from the open folder of the worktree, and no part
//! that is a link is passed through.
//!
//! I/O: opens, reads, writes and removes files below one folder.

use std::path::{Component, Path, PathBuf};

/// The path of a file inside a worktree as `git` printed it.
#[cfg(unix)]
#[must_use]
pub fn path(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
#[must_use]
pub fn path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// `path` names a file below the worktree and nothing of a Git folder: relative, only normal
/// parts, no `.git`.
#[must_use]
pub fn inside(path: &Path) -> bool {
    let mut parts = path.components().peekable();
    parts.peek().is_some()
        && parts.all(
            |part| matches!(part, Component::Normal(name) if !name.eq_ignore_ascii_case(".git")),
        )
}

#[cfg(unix)]
pub use held::Root;

#[cfg(unix)]
mod held {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::os::fd::OwnedFd;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use rustix::fs::{AtFlags, Mode, OFlags, mkdirat, openat, readlinkat, symlinkat, unlinkat};

    const FOLDER: OFlags = OFlags::DIRECTORY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);
    const FILE: OFlags = OFlags::NOFOLLOW
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);

    /// Makes the folder `part` in the open folder `at` and opens it. A link or a file that is
    /// in the way goes first; a link is never followed.
    fn made(at: &OwnedFd, part: &OsStr) -> io::Result<OwnedFd> {
        let _ = unlinkat(at, part, AtFlags::empty());
        match mkdirat(at, part, Mode::from_raw_mode(0o755)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(failed) => return Err(failed.into()),
        }
        Ok(openat(at, part, FOLDER, Mode::empty())?)
    }

    /// The open folder of a worktree. Each path is found from it, part by part.
    #[derive(Debug)]
    pub struct Root(OwnedFd);

    impl Root {
        /// Opens the folder of a worktree; a link at its place is not opened.
        ///
        /// # Errors
        /// The error of the system.
        pub fn open(folder: &Path) -> io::Result<Self> {
            Ok(Self(openat(
                rustix::fs::CWD,
                folder,
                FOLDER,
                Mode::empty(),
            )?))
        }

        /// The open folder that holds the last part of `path`, and that part. With `make`, a
        /// folder that is missing is made, and a file or a link in the place of a folder is
        /// removed first. Without it, `None` when a part is no folder.
        fn above<'a>(
            &self,
            path: &'a Path,
            make: bool,
        ) -> io::Result<Option<(OwnedFd, &'a OsStr)>> {
            let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
            let mut at = self.0.try_clone()?;
            for part in path.parent().into_iter().flat_map(Path::components) {
                let part = part.as_os_str();
                let next = match openat(&at, part, FOLDER, Mode::empty()) {
                    Ok(next) => next,
                    Err(_) if make => made(&at, part)?,
                    Err(_) => return Ok(None),
                };
                at = next;
            }
            Ok(Some((at, name)))
        }

        /// Copies the file or the link at `path` of the worktree to the same path below
        /// `into`, a folder of the runner. A link is copied as a link, never followed. `false`
        /// when there is no file and no link at the path.
        ///
        /// # Errors
        /// The error of the system when the copy cannot be written.
        pub fn take(&self, path: &Path, into: &Path) -> io::Result<bool> {
            let Some((above, name)) = self.above(path, false)? else {
                return Ok(false);
            };
            let copy = into.join(path);
            if let Some(folder) = copy.parent() {
                std::fs::create_dir_all(folder)?;
            }
            let Ok(found) = openat(&above, name, FILE | OFlags::RDONLY, Mode::empty()) else {
                // Not opened: a link (its text is the copy), or nothing.
                return match readlinkat(&above, name, Vec::new()) {
                    Ok(to) => {
                        let to: &OsStr = std::os::unix::ffi::OsStrExt::from_bytes(to.to_bytes());
                        std::os::unix::fs::symlink(to, &copy).map(|()| true)
                    }
                    Err(_) => Ok(false),
                };
            };
            let mut found = File::from(found);
            let about = found.metadata()?;
            if !about.is_file() {
                return Ok(false);
            }
            let mut written = File::create(&copy)?;
            io::copy(&mut found, &mut written)?;
            let runs = about.permissions().mode() & 0o111 != 0;
            let mode = if runs { 0o755 } else { 0o644 };
            written.set_permissions(std::fs::Permissions::from_mode(mode))?;
            Ok(true)
        }

        /// Writes the file or the link at `path` below `from`, a folder of the runner, to the
        /// same path of the worktree. What was at the path is removed first; the new file is
        /// made new, so a link put there in between is not written through.
        ///
        /// # Errors
        /// The error of the system.
        pub fn put(&self, path: &Path, from: &Path) -> io::Result<()> {
            let source = from.join(path);
            let about = std::fs::symlink_metadata(&source)?;
            let (above, name) = self.above(path, true)?.ok_or(io::ErrorKind::InvalidInput)?;
            if unlinkat(&above, name, AtFlags::empty()).is_err() {
                // A folder at the place of a file: it goes when it is empty.
                let _ = unlinkat(&above, name, AtFlags::REMOVEDIR);
            }
            if about.is_symlink() {
                let to = std::fs::read_link(&source)?;
                return Ok(symlinkat(to, &above, name)?);
            }
            let runs = about.permissions().mode() & 0o111 != 0;
            let mode = Mode::from_raw_mode(if runs { 0o755 } else { 0o644 });
            let new = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | FILE;
            let mut written = File::from(openat(&above, name, new, mode)?);
            io::copy(&mut File::open(&source)?, &mut written)?;
            Ok(())
        }

        /// Removes the file or the link at `path` of the worktree, and each folder above it
        /// that is empty after. Nothing at the path is the wanted state.
        ///
        /// # Errors
        /// The error of the system for a file that is there and cannot be removed.
        pub fn remove(&self, path: &Path) -> io::Result<()> {
            let Some((above, name)) = self.above(path, false)? else {
                return Ok(());
            };
            match unlinkat(&above, name, AtFlags::empty()) {
                Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                Err(failed) => return Err(failed.into()),
            }
            let mut folder = path.parent();
            while let Some(empty) = folder.filter(|folder| folder.file_name().is_some()) {
                let Some((above, name)) = self.above(empty, false)? else {
                    break;
                };
                if unlinkat(&above, name, AtFlags::REMOVEDIR).is_err() {
                    break;
                }
                folder = empty.parent();
            }
            Ok(())
        }
    }
}

/// A system where a path cannot be walked without following a link has no safe copy yet.
#[cfg(not(unix))]
#[derive(Debug)]
pub struct Root;

#[cfg(not(unix))]
impl Root {
    fn none<T>() -> std::io::Result<T> {
        Err(std::io::Error::other(
            "the files of a worktree cannot be read safely on this system yet",
        ))
    }

    pub fn open(_folder: &Path) -> std::io::Result<Self> {
        Self::none()
    }

    pub fn take(&self, _path: &Path, _into: &Path) -> std::io::Result<bool> {
        Self::none()
    }

    pub fn put(&self, _path: &Path, _from: &Path) -> std::io::Result<()> {
        Self::none()
    }

    pub fn remove(&self, _path: &Path) -> std::io::Result<()> {
        Self::none()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn folders() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let keep = tempfile::tempdir().expect("a folder");
        let top = std::fs::canonicalize(keep.path()).expect("its real path");
        let (worktree, own, outside) = (top.join("worktree"), top.join("own"), top.join("outside"));
        for folder in [&worktree, &own, &outside] {
            std::fs::create_dir_all(folder).expect("a folder");
        }
        std::fs::write(outside.join("secret"), "SECRET\n").expect("a file");
        (keep, worktree, own, outside)
    }

    #[test]
    fn a_path_with_dots_or_a_git_folder_is_not_inside() {
        for bad in [
            "",
            "../x",
            "/etc/hosts",
            "a/../b",
            ".git/config",
            "a/.GIT/x",
            "./a",
        ] {
            assert!(!inside(Path::new(bad)), "{bad}");
        }
        assert!(inside(Path::new("src/a file.rs")));
        assert!(inside(Path::new("-odd:name")));
    }

    #[test]
    fn a_link_is_copied_as_a_link_and_a_folder_that_is_a_link_is_not_passed() {
        let (_keep, worktree, own, outside) = folders();
        std::fs::write(worktree.join("plain.txt"), "plain\n").expect("a file");
        std::os::unix::fs::symlink(outside.join("secret"), worktree.join("link")).expect("a link");
        std::os::unix::fs::symlink(&outside, worktree.join("dir")).expect("a link");
        let root = Root::open(&worktree).expect("the worktree");
        assert!(root.take(Path::new("plain.txt"), &own).expect("a copy"));
        assert!(root.take(Path::new("link"), &own).expect("a copy"));
        assert!(own.join("link").is_symlink(), "the link was followed");
        // `dir` is a link: nothing below it is a file of the worktree.
        assert!(!root.take(Path::new("dir/secret"), &own).expect("no copy"));
        assert!(!own.join("dir").exists());
        assert!(!root.take(Path::new("gone"), &own).expect("no copy"));
        assert!(Root::open(&worktree.join("dir")).is_err());
    }

    #[test]
    fn a_file_is_never_written_or_removed_through_a_link() {
        let (_keep, worktree, own, outside) = folders();
        std::fs::create_dir_all(own.join("d")).expect("a folder");
        std::fs::write(own.join("d/f"), "PAYLOAD\n").expect("a file");
        std::fs::write(own.join("top"), "PAYLOAD\n").expect("a file");
        // What a process of an agent leaves: a link where a folder and where a file should be.
        std::os::unix::fs::symlink(&outside, worktree.join("d")).expect("a link");
        std::os::unix::fs::symlink(outside.join("secret"), worktree.join("top")).expect("a link");
        let root = Root::open(&worktree).expect("the worktree");
        root.put(Path::new("d/f"), &own).expect("a write");
        root.put(Path::new("top"), &own).expect("a write");
        assert!(!outside.join("f").exists(), "a file was written outside");
        let secret = std::fs::read_to_string(outside.join("secret")).expect("the file");
        assert_eq!(
            secret, "SECRET\n",
            "a file outside was written through a link"
        );
        assert_eq!(
            std::fs::read_to_string(worktree.join("d/f")).expect("the file"),
            "PAYLOAD\n"
        );
        assert!(
            worktree
                .join("d")
                .symlink_metadata()
                .expect("a folder")
                .is_dir()
        );
        std::fs::remove_dir_all(worktree.join("d")).expect("the folder");
        std::os::unix::fs::symlink(&outside, worktree.join("d")).expect("a link");
        root.remove(Path::new("d/secret"))
            .expect("nothing to remove");
        assert!(
            outside.join("secret").exists(),
            "a file outside was removed"
        );
        root.remove(Path::new("top")).expect("the removal");
        assert!(!worktree.join("top").exists());
    }

    #[test]
    fn a_removed_file_takes_its_empty_folders_with_it() {
        let (_keep, worktree, _own, _outside) = folders();
        std::fs::create_dir_all(worktree.join("a/b")).expect("folders");
        std::fs::write(worktree.join("a/b/f"), "x").expect("a file");
        std::fs::write(worktree.join("a/keep"), "x").expect("a file");
        let root = Root::open(&worktree).expect("the worktree");
        root.remove(Path::new("a/b/f")).expect("the removal");
        assert!(!worktree.join("a/b").exists());
        assert!(worktree.join("a/keep").exists());
    }
}
