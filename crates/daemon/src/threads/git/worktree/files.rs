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

use super::super::place::walk::{Dir, Make};

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

/// Whether a file with these properties can run.
#[cfg(unix)]
fn runs(about: &std::fs::Metadata) -> bool {
    std::os::unix::fs::PermissionsExt::mode(&about.permissions()) & 0o111 != 0
}

#[cfg(not(unix))]
const fn runs(_about: &std::fs::Metadata) -> bool {
    false
}

/// Makes a link with the text `to` at `at`, a path in a folder of the runner.
#[cfg(unix)]
fn link(to: &std::ffi::OsStr, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(to, at)
}

#[cfg(not(unix))]
fn link(_to: &std::ffi::OsStr, _at: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// The open folder of a worktree. Each path is found from it, part by part.
#[derive(Debug)]
pub struct Root(Dir);

impl Root {
    /// Opens the folder of a worktree; a link at its place is not opened.
    ///
    /// # Errors
    /// The error of the system.
    pub fn open(folder: &Path) -> std::io::Result<Self> {
        Dir::open(folder).map(Self)
    }

    /// Copies the file or the link at `path` of the worktree to the same path below `into`,
    /// a folder of the runner. A link is copied as a link, never followed. `false` when
    /// there is no file and no link at the path. At most `left` bytes are copied; `left` gets
    /// smaller by what was copied.
    ///
    /// # Errors
    /// The error of the system when the copy cannot be written, or the file is larger than
    /// `left`.
    pub fn take(&self, path: &Path, into: &Path, left: &mut u64) -> std::io::Result<bool> {
        use std::io::Read as _;
        let Some((above, name)) = self.0.above(path, Make::No)? else {
            return Ok(false);
        };
        let copy = into.join(path);
        if let Some(folder) = copy.parent() {
            std::fs::create_dir_all(folder)?;
        }
        if let Some(to) = above.link(name) {
            return link(&to, &copy).map(|()| true);
        }
        let Some(found) = above.file(name)? else {
            return Ok(false);
        };
        let runs = runs(&found.metadata()?);
        let mut written = std::fs::File::create(&copy)?;
        // One byte more than what is left shows a file that is too large. A file with a
        // hole is read as the bytes that it says it has.
        let taken = std::io::copy(&mut found.take(left.saturating_add(1)), &mut written)?;
        *left = left.checked_sub(taken).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "the changed files are larger than what the checkpoints of this chat can still \
                 hold; remove old checkpoints of the chat or make the files smaller",
            )
        })?;
        if runs {
            let mut can = written.metadata()?.permissions();
            #[cfg(unix)]
            std::os::unix::fs::PermissionsExt::set_mode(&mut can, 0o755);
            written.set_permissions(can)?;
        }
        Ok(true)
    }

    /// Writes the file or the link at `path` below `from`, a folder of the runner, to the
    /// same path of the worktree. What was at the path is removed first; the new file is made
    /// new, so a link put there in between is not written through.
    ///
    /// # Errors
    /// The error of the system.
    pub fn put(&self, path: &Path, from: &Path) -> std::io::Result<()> {
        let source = from.join(path);
        let about = std::fs::symlink_metadata(&source)?;
        let (above, name) = self
            .0
            .above(path, Make::Over)?
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        if above.remove(name, false).is_err() {
            // A folder at the place of a file: it goes when it is empty.
            let _ = above.remove(name, true);
        }
        if about.is_symlink() {
            return above.new_link(name, std::fs::read_link(&source)?.as_os_str());
        }
        let mut written = above.new_file(name, runs(&about))?;
        std::io::copy(&mut std::fs::File::open(&source)?, &mut written)?;
        Ok(())
    }

    /// Removes the file or the link at `path` of the worktree, and each folder above it that
    /// is empty after. Nothing at the path is the wanted state.
    ///
    /// # Errors
    /// The error of the system for a file that is there and cannot be removed.
    pub fn remove(&self, path: &Path) -> std::io::Result<()> {
        let Some((above, name)) = self.0.above(path, Make::No)? else {
            return Ok(());
        };
        above.remove(name, false)?;
        let mut folder = path.parent();
        while let Some(empty) = folder.filter(|folder| folder.file_name().is_some()) {
            let Some((above, name)) = self.0.above(empty, Make::No)? else {
                break;
            };
            if above.remove(name, true).is_err() {
                break;
            }
            folder = empty.parent();
        }
        Ok(())
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
        assert!(
            root.take(Path::new("plain.txt"), &own, &mut { u64::MAX })
                .expect("a copy")
        );
        assert!(
            root.take(Path::new("link"), &own, &mut { u64::MAX })
                .expect("a copy")
        );
        assert!(own.join("link").is_symlink(), "the link was followed");
        // `dir` is a link: nothing below it is a file of the worktree.
        assert!(
            !root
                .take(Path::new("dir/secret"), &own, &mut { u64::MAX })
                .expect("no copy")
        );
        assert!(!own.join("dir").exists());
        assert!(
            !root
                .take(Path::new("gone"), &own, &mut { u64::MAX })
                .expect("no copy")
        );
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
