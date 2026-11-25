//! A folder that is held open, and paths below it walked one part at a time with no link
//! followed (P12, THR-050). An agent can put a link in the place of each file and each folder
//! that it can write, at any moment. A path that is checked and then used by name can be led
//! elsewhere in between; a path that is walked from an open folder cannot: each part is
//! opened in the folder that is already open, and a part that is a link is not passed.
//!
//! The files of a worktree ([`super::super::worktree::files`]) and the files of the Git
//! folder of the person ([`super::super::project`]) are read and written through it.
//!
//! I/O: opens, reads, writes and removes below one folder.

use std::ffi::OsStr;

/// What to do when a folder on the way is not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Make {
    /// Nothing: the path is then not there.
    No,
    /// Make the folder. A file or a link in its place is an error.
    Missing,
    /// Make the folder; a file or a link in its place is removed first.
    Over,
}

/// The name of a file as text, for a file name that the runner chose itself.
#[must_use]
pub fn name(text: &str) -> &OsStr {
    OsStr::new(text)
}

#[cfg(unix)]
pub use held::Dir;

#[cfg(unix)]
mod held {
    use std::ffi::{OsStr, OsString};
    use std::fs::File;
    use std::io::{self, Read as _};
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
    use std::path::Path;

    use rustix::fs::{
        AtFlags, Mode, OFlags, mkdirat, openat, readlinkat, renameat, symlinkat, unlinkat,
    };
    use rustix::io::Errno;

    use super::Make;

    const FOLDER: OFlags = OFlags::DIRECTORY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);
    const FILE: OFlags = OFlags::NOFOLLOW
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);

    /// The most folders in folders that [`Dir::remove_all`] goes into.
    const DEEPEST: usize = 128;

    /// A folder that is held open.
    #[derive(Debug)]
    pub struct Dir(OwnedFd);

    impl Dir {
        pub(in super::super) const fn from_fd(fd: OwnedFd) -> Self {
            Self(fd)
        }

        /// Makes prior entry changes in this directory durable.
        ///
        /// # Errors
        /// The filesystem could not synchronize the directory.
        pub fn sync(&self) -> io::Result<()> {
            Ok(rustix::fs::fsync(&self.0)?)
        }

        /// Opens the folder at `folder`; a link at its place is not opened.
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

        /// The folder `part` of this folder, opened or made as `make` says.
        fn step(&self, part: &OsStr, make: Make) -> io::Result<Option<Self>> {
            match openat(&self.0, part, FOLDER, Mode::empty()) {
                Ok(next) => return Ok(Some(Self(next))),
                // Nothing, a link or a file at the name: no folder. Each other failure (no
                // right to open it) is a failure: what is below a folder that is there and
                // is not read would be taken as gone.
                // A name that is too long for the disk cannot be on it.
                Err(
                    Errno::NOENT | Errno::NOTDIR | Errno::LOOP | Errno::MLINK | Errno::NAMETOOLONG,
                ) if make == Make::No => {
                    return Ok(None);
                }
                Err(failed) if make == Make::No => return Err(failed.into()),
                Err(_) => {}
            }
            if make == Make::Over {
                // A link or a file is in the way: it goes, a link is never followed.
                let _ = unlinkat(&self.0, part, AtFlags::empty());
            }
            match mkdirat(&self.0, part, Mode::from_raw_mode(0o755)) {
                Ok(()) => self.sync()?,
                Err(Errno::EXIST) => {}
                Err(failed) => return Err(failed.into()),
            }
            Ok(Some(Self(openat(&self.0, part, FOLDER, Mode::empty())?)))
        }

        /// The folder at `path` below this one; `None` when a part is no folder and `make`
        /// makes none.
        ///
        /// # Errors
        /// The error of the system when a folder cannot be made or opened.
        pub fn below(&self, path: &Path, make: Make) -> io::Result<Option<Self>> {
            let mut at = Self(self.0.try_clone()?);
            for part in path.components() {
                match at.step(part.as_os_str(), make)? {
                    Some(next) => at = next,
                    None => return Ok(None),
                }
            }
            Ok(Some(at))
        }

        /// The open folder that holds the last part of `path`, and that part.
        ///
        /// # Errors
        /// As [`Dir::below`]; a path with no last part.
        pub fn above<'a>(
            &self,
            path: &'a Path,
            make: Make,
        ) -> io::Result<Option<(Self, &'a OsStr)>> {
            let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
            let folder = path.parent().unwrap_or_else(|| Path::new(""));
            Ok(self.below(folder, make)?.map(|above| (above, name)))
        }

        /// Opens the file `name` of this folder to read it. `None` when there is nothing, a
        /// link, a folder or a pipe at the name.
        ///
        /// # Errors
        /// The error of the system for a file that is there and cannot be read.
        pub fn file(&self, name: &OsStr) -> io::Result<Option<File>> {
            let found = match openat(&self.0, name, FILE | OFlags::RDONLY, Mode::empty()) {
                Ok(found) => found,
                // Nothing, a link, or a part of the path that is no folder: no file. Each
                // other failure (no right to read it) is a failure, not "no file": a file
                // that is there and is not read would be missing from what is saved.
                Err(
                    Errno::NOENT
                    | Errno::LOOP
                    | Errno::MLINK
                    | Errno::NOTDIR
                    | Errno::NXIO
                    | Errno::OPNOTSUPP
                    | Errno::NAMETOOLONG,
                ) => {
                    return Ok(None);
                }
                Err(failed) => return Err(failed.into()),
            };
            let found = File::from(found);
            Ok(found.metadata()?.is_file().then_some(found))
        }

        /// Reads the file `name` when it has at most `most` bytes; `None` as [`Dir::file`],
        /// and for a larger file.
        ///
        /// # Errors
        /// As [`Dir::file`].
        pub fn read(&self, name: &OsStr, most: u64) -> io::Result<Option<Vec<u8>>> {
            let Some(found) = self.file(name)? else {
                return Ok(None);
            };
            if found.metadata()?.len() > most {
                return Ok(None);
            }
            let mut bytes = Vec::new();
            found.take(most).read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }

        /// The text of the link `name`; `None` when it is no link.
        #[must_use]
        pub fn link(&self, name: &OsStr) -> Option<OsString> {
            let to = readlinkat(&self.0, name, Vec::new()).ok()?;
            Some(OsString::from_vec(to.into_bytes()))
        }

        /// Makes the file `name` new and opens it to write. A file or a link that is there
        /// already is an error (`AlreadyExists`), never written through. `runs` makes a file
        /// that can run.
        ///
        /// # Errors
        /// The error of the system.
        pub fn new_file(&self, name: &OsStr, runs: bool) -> io::Result<File> {
            let mode = Mode::from_raw_mode(if runs { 0o755 } else { 0o644 });
            let new = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | FILE;
            Ok(File::from(openat(&self.0, name, new, mode)?))
        }

        /// Makes the link `name` with the text `to`.
        ///
        /// # Errors
        /// The error of the system.
        pub fn new_link(&self, name: &OsStr, to: &OsStr) -> io::Result<()> {
            Ok(symlinkat(to, &self.0, name)?)
        }

        /// Makes the folder `name` new; one that is there is an error (`AlreadyExists`).
        ///
        /// # Errors
        /// The error of the system.
        pub fn new_folder(&self, name: &OsStr) -> io::Result<()> {
            Ok(mkdirat(&self.0, name, Mode::from_raw_mode(0o755))?)
        }

        /// Gives the entry `from` of this folder the name `to`; what was at `to` is replaced.
        ///
        /// # Errors
        /// The error of the system.
        pub fn rename(&self, from: &OsStr, to: &OsStr) -> io::Result<()> {
            Ok(renameat(&self.0, from, &self.0, to)?)
        }

        /// Removes the file or the link `name`; a folder with `folder`. Nothing at the name
        /// is the wanted state.
        ///
        /// # Errors
        /// The error of the system for an entry that is there and cannot be removed.
        pub fn remove(&self, name: &OsStr, folder: bool) -> io::Result<()> {
            let how = if folder {
                AtFlags::REMOVEDIR
            } else {
                AtFlags::empty()
            };
            match unlinkat(&self.0, name, how) {
                Ok(()) | Err(Errno::NOENT | Errno::NAMETOOLONG) => Ok(()),
                Err(failed) => Err(failed.into()),
            }
        }

        /// The names of the entries of this folder.
        ///
        /// # Errors
        /// The error of the system.
        pub fn names(&self) -> io::Result<Vec<OsString>> {
            let mut names = Vec::new();
            for entry in rustix::fs::Dir::read_from(&self.0)? {
                let name = OsStr::from_bytes(entry?.file_name().to_bytes()).to_owned();
                if name != "." && name != ".." {
                    names.push(name);
                }
            }
            Ok(names)
        }

        /// Removes the folder `name` with all that it holds. A link in it is removed, never
        /// followed. A tree of folders deeper than [`DEEPEST`] is refused: an agent can make one.
        ///
        /// # Errors
        /// The error of the system.
        pub fn remove_all(&self, name: &OsStr) -> io::Result<()> {
            self.remove_tree(name, DEEPEST)
        }

        fn remove_tree(&self, name: &OsStr, left: usize) -> io::Result<()> {
            let Some(inside) = self.step(name, Make::No)? else {
                return self.remove(name, false);
            };
            let left = left
                .checked_sub(1)
                .ok_or_else(|| io::Error::other("the folders are too deep to remove"))?;
            for entry in inside.names()? {
                // A file or a link goes at once; what stays is a folder.
                let gone = inside.remove(&entry, false);
                gone.or_else(|_| inside.remove_tree(&entry, left))?;
            }
            self.remove(name, true)
        }
    }
}

/// A system where a path cannot be walked without following a link has no safe access yet:
/// each use answers with an error.
#[cfg(not(unix))]
#[derive(Debug)]
pub struct Dir;

#[cfg(not(unix))]
mod none {
    use std::ffi::{OsStr, OsString};
    use std::fs::File;
    use std::io;
    use std::path::Path;

    use super::{Dir, Make};

    fn none<T>() -> io::Result<T> {
        Err(io::Error::other(
            "files cannot be read or written safely on this system yet",
        ))
    }

    impl Dir {
        pub fn sync(&self) -> io::Result<()> {
            none()
        }

        pub fn open(_folder: &Path) -> io::Result<Self> {
            none()
        }

        pub fn below(&self, _path: &Path, _make: Make) -> io::Result<Option<Self>> {
            none()
        }

        pub fn above<'a>(
            &self,
            _path: &'a Path,
            _make: Make,
        ) -> io::Result<Option<(Self, &'a OsStr)>> {
            none()
        }

        pub fn file(&self, _name: &OsStr) -> io::Result<Option<File>> {
            none()
        }

        pub fn read(&self, _name: &OsStr, _most: u64) -> io::Result<Option<Vec<u8>>> {
            none()
        }

        #[must_use]
        pub fn link(&self, _name: &OsStr) -> Option<OsString> {
            None
        }

        pub fn new_file(&self, _name: &OsStr, _runs: bool) -> io::Result<File> {
            none()
        }

        pub fn new_link(&self, _name: &OsStr, _to: &OsStr) -> io::Result<()> {
            none()
        }

        pub fn new_folder(&self, _name: &OsStr) -> io::Result<()> {
            none()
        }

        pub fn rename(&self, _from: &OsStr, _to: &OsStr) -> io::Result<()> {
            none()
        }

        pub fn remove(&self, _name: &OsStr, _folder: bool) -> io::Result<()> {
            none()
        }

        pub fn names(&self) -> io::Result<Vec<OsString>> {
            none()
        }

        pub fn remove_all(&self, _name: &OsStr) -> io::Result<()> {
            none()
        }
    }
}
