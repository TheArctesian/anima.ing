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

use std::io::BufRead as _;
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

/// A path of the worktree for a text that a person reads: no control character of the name
/// is shown as it is, and a long name is cut.
#[must_use]
pub fn shown(path: &Path) -> String {
    let whole = path.to_string_lossy();
    let (count, most) = (whole.chars().count(), 200);
    // The start and the end of a long name are kept: the end is the name of the file.
    let kept: String = if count > most {
        let end = whole.chars().skip(count - most / 2);
        whole
            .chars()
            .take(most / 2)
            .chain("...".chars())
            .chain(end)
            .collect()
    } else {
        whole.into_owned()
    };
    kept.escape_debug().collect()
}

/// The least bytes that one file of a snapshot costs: a block of the disk.
const BLOCK: u64 = 4096;

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
    pub fn take(&self, path: &Path, into: &Dir, left: &mut u64) -> std::io::Result<bool> {
        match self.take_to(path, (into, path), left) {
            // Two names of the worktree that differ only in the case of a letter are one
            // name on a disk that does not tell them apart: the copy is there. Two
            // different files with such names cannot both be kept there.
            Err(there) if there.kind() == std::io::ErrorKind::AlreadyExists => {
                if self.same(path, into)? {
                    return Ok(true);
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "the file {} and a different file have names that differ only in the \
                         case of a letter; give one of them a different name",
                        shown(path)
                    ),
                ))
            }
            taken => taken,
        }
    }

    /// Whether the copy at `path` below `into` has the content of the file or the link at
    /// `path` of the worktree.
    fn same(&self, path: &Path, into: &Dir) -> std::io::Result<bool> {
        let (Some((above, name)), Some((copies, last))) =
            (self.0.above(path, Make::No)?, into.above(path, Make::No)?)
        else {
            return Ok(false);
        };
        if let Some(to) = above.link(name) {
            return Ok(copies.link(last) == Some(to));
        }
        let (Some(ours), Some(theirs)) = (above.file(name)?, copies.file(last)?) else {
            return Ok(false);
        };
        if ours.metadata()?.len() != theirs.metadata()?.len() {
            return Ok(false);
        }
        let (mut ours, mut theirs) = (
            std::io::BufReader::new(ours),
            std::io::BufReader::new(theirs),
        );
        loop {
            let (one, other) = (ours.fill_buf()?, theirs.fill_buf()?);
            let most = one.len().min(other.len());
            if one[..most] != other[..most] {
                return Ok(false);
            }
            if most == 0 {
                return Ok(one.len() == other.len());
            }
            ours.consume(most);
            theirs.consume(most);
        }
    }

    /// As [`Root::take`], with the place of the copy named: the path `at` below `into`, the
    /// open folder of the runner. The copy is made new, one path part at a time: a link that
    /// an earlier copy put on the way (a link of the worktree is copied as a link) is never
    /// written through, and a copy never takes the place of a different one.
    ///
    /// # Errors
    /// As [`Root::take`]; `AlreadyExists` when a copy is at `at` already.
    pub fn take_to(
        &self,
        path: &Path,
        (into, at): (&Dir, &Path),
        left: &mut u64,
    ) -> std::io::Result<bool> {
        use std::io::Read as _;
        let Some((above, name)) = self.0.above(path, Make::No)? else {
            return Ok(false);
        };
        let too_large = || {
            std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "the changed files are larger than what the checkpoints of this chat can still \
                 hold; remove old checkpoints of the chat or make the files smaller",
            )
        };
        // Each file and each link uses at least one block of the disk of the records.
        let pay = |left: &mut u64, bytes: u64| {
            *left = left.checked_sub(bytes.max(BLOCK)).ok_or_else(too_large)?;
            Ok::<_, std::io::Error>(())
        };
        let link_text = above.link(name);
        let found = match &link_text {
            Some(_) => None,
            None => above.file(name)?,
        };
        if link_text.is_none() && found.is_none() {
            return Ok(false);
        }
        let (copies, last) = into
            .above(at, Make::Missing)?
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        if let Some(to) = link_text {
            pay(left, u64::try_from(to.len()).unwrap_or(u64::MAX))?;
            return copies.new_link(last, &to).map(|()| true);
        }
        let Some(found) = found else {
            return Ok(false);
        };
        let about = found.metadata()?;
        // A file that says a size over what is left is not read at all.
        if about.len() > *left {
            return Err(too_large());
        }
        let mut written = copies.new_file(last, runs(&about))?;
        // One byte more than what is left shows a file that grew while it was read.
        let taken = std::io::copy(&mut found.take(left.saturating_add(1)), &mut written)?;
        pay(left, taken)?;
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
            // The file is gone; an empty folder that cannot be reached stays.
            let Ok(Some((above, name))) = self.0.above(empty, Make::No) else {
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
mod tests;
