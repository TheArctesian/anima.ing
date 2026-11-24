//! The refs of the repository of the person as files: `HEAD`, the loose refs and `packed-refs`.
//! The runner reads them and moves a branch of a thread here with no `git`, so that no hook of
//! the repository runs for it (`reference-transaction`).
//!
//! Each file is found from the open Git folder, one path part at a time, with no link followed
//! ([`walk`]): an agent can put a link in the place of a ref or of a folder of refs at any
//! moment.
//!
//! [`id`], [`name`] and [`in_packed`] are pure; the rest reads and writes files.

use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::path::Path;

use super::super::place::walk::{self, Dir, Make};
use super::{Layout, ProjectError};

/// The most bytes of `packed-refs` that are read.
const MOST_PACKED: u64 = 64 * 1024 * 1024;

/// The most refs that one name is followed through.
const MOST_STEPS: usize = 5;

/// `text` when it is the ID of an object: 40 or 64 hex digits in lower case.
#[must_use]
pub fn id(text: &str) -> Option<&str> {
    let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
    (matches!(text.len(), 40 | 64) && text.chars().all(hex)).then_some(text)
}

/// `text` when it is the full name of a ref that is safe as a path below the Git folder:
/// `refs/` and then the alphabet of a branch name, with no `..` and no part that hides.
#[must_use]
pub fn name(text: &str) -> Option<&str> {
    name_of(text)
}

/// [`name`], for the places where a local value has that name.
fn name_of(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("refs/")?;
    crate::threads::git::place::branch_name(rest)
        .is_ok()
        .then_some(text)
}

/// What a ref file holds.
enum Held {
    Id(String),
    Ref(String),
}

/// The most bytes of the file of a loose ref or of `HEAD`.
const SMALL: u64 = 4096;

/// Opens a Git folder to walk below it with no link followed.
fn open(folder: &Path) -> Result<Dir, ProjectError> {
    Ok(Dir::open(folder)?)
}

/// Reads the file of a ref at `path` below `dir`; `None` when there is no file (a folder is
/// none too). A link in the place of the file or of a folder above it is never followed.
fn loose(dir: &Dir, path: &Path) -> Result<Option<Held>, ProjectError> {
    let Some((above, name)) = dir.above(path, Make::No)? else {
        return Ok(None);
    };
    let Some(bytes) = above.read(name, SMALL)? else {
        // Nothing or a folder is no ref; a link or a large file is not one that git wrote.
        let odd = above.link(name).is_some() || above.file(name)?.is_some();
        return if odd {
            Err(ProjectError::Odd("a ref"))
        } else {
            Ok(None)
        };
    };
    let text = String::from_utf8(bytes).map_err(|_| ProjectError::Odd("a ref"))?;
    let text = text.trim_end();
    if let Some(to) = text.strip_prefix("ref: ") {
        return name_of(to)
            .map(|to| Some(Held::Ref(to.to_owned())))
            .ok_or(ProjectError::Odd("a ref that names a ref"));
    }
    id(text)
        .map(|id| Some(Held::Id(id.to_owned())))
        .ok_or(ProjectError::Odd("a ref"))
}

/// The ID that `packed` (the text of `packed-refs`) gives for the ref `wanted`.
#[must_use]
pub fn in_packed<'a>(packed: &'a str, wanted: &str) -> Option<&'a str> {
    packed
        .lines()
        .filter(|line| !line.starts_with(['#', '^']))
        .filter_map(|line| line.split_once(' '))
        .find(|(_, name)| *name == wanted)
        .and_then(|(found, _)| id(found))
}

/// Where the ref `wanted` of the shared Git folder points, not followed: loose, else packed.
fn held(common: &Dir, wanted: &str) -> Result<Option<Held>, ProjectError> {
    if let Some(found) = loose(common, Path::new(wanted))? {
        return Ok(Some(found));
    }
    let Some(packed) = common.read(walk::name("packed-refs"), MOST_PACKED)? else {
        return Ok(None);
    };
    let packed = String::from_utf8_lossy(&packed);
    Ok(in_packed(&packed, wanted).map(|id| Held::Id(id.to_owned())))
}

/// The ID of the object that the ref `wanted` points to; `None` when there is no such ref.
///
/// # Errors
/// A [`ProjectError`] for a name that is not safe or a file that is not a ref.
pub fn resolve(common: &Path, wanted: &str) -> Result<Option<String>, ProjectError> {
    let mut at = name(wanted).ok_or(ProjectError::Escapes)?.to_owned();
    let common = &open(common)?;
    for _ in 0..MOST_STEPS {
        match held(common, &at)? {
            Some(Held::Id(id)) => return Ok(Some(id)),
            Some(Held::Ref(next)) => at = next,
            None => return Ok(None),
        }
    }
    Err(ProjectError::Odd("a ref that names a ref"))
}

/// The ref that the ref `wanted` names (as `refs/remotes/origin/HEAD` does); `None` when it
/// holds an ID or is not there.
///
/// # Errors
/// As [`resolve`].
pub fn names(common: &Path, wanted: &str) -> Result<Option<String>, ProjectError> {
    let wanted = name(wanted).ok_or(ProjectError::Escapes)?;
    Ok(match held(&open(common)?, wanted)? {
        Some(Held::Ref(to)) => Some(to),
        _ => None,
    })
}

/// Where a checkout is: its branch, and the commit of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Head {
    /// `None` for a detached `HEAD`, or a `HEAD` on a ref that is no branch.
    pub branch: Option<String>,
    /// `None` for a branch with no commit yet.
    pub commit: Option<String>,
}

/// Reads the `HEAD` of the checkout of `layout`.
///
/// # Errors
/// A [`ProjectError`] when `HEAD` or the ref it names is not as git writes it.
pub fn head(layout: &Layout) -> Result<Head, ProjectError> {
    match loose(&layout.entry()?, Path::new("HEAD"))? {
        Some(Held::Id(commit)) => Ok(Head {
            branch: None,
            commit: Some(commit),
        }),
        Some(Held::Ref(on)) => Ok(Head {
            branch: on.strip_prefix("refs/heads/").map(str::to_owned),
            commit: resolve(&layout.common_dir, &on)?,
        }),
        None => Err(ProjectError::Odd("HEAD")),
    }
}

/// What a ref must be before it is moved.
#[derive(Clone, Copy, Debug)]
pub enum Was<'a> {
    Absent,
    At(&'a str),
    /// A ref of the runner alone, which nothing else moves.
    Any,
}

/// Writes `text` to the file `name` of the open folder `above` as git does: to `<name>.lock`,
/// made new, then moved into place. `check` runs while the lock is held. A link at the place
/// of the file is replaced, never followed, and no folder above it is a link: the folder is
/// held open.
fn replace(
    above: &Dir,
    name: &OsStr,
    text: &str,
    check: impl FnOnce() -> Result<(), ProjectError>,
) -> Result<(), ProjectError> {
    let mut lock = name.to_owned();
    lock.push(".lock");
    let mut held = above
        .new_file(&lock, false)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => {
                ProjectError::Locked(lock.to_string_lossy().into_owned())
            }
            _ => ProjectError::Io(error),
        })?;
    let written = check().and_then(|()| {
        held.write_all(text.as_bytes())?;
        held.sync_all()?;
        Ok(above.rename(&lock, name)?)
    });
    if written.is_err() {
        // The lock is of this call alone; the first failure is the answer.
        let _ = above.remove(&lock, false);
    }
    written
}

/// Points the ref `wanted` of the shared Git folder at `to`, only when it is as `was` says.
/// No reflog line is written.
///
/// # Errors
/// [`ProjectError::Moved`] when the ref is not as `was` says; [`ProjectError::Locked`] when
/// git holds the ref; [`ProjectError::Escapes`] for a name that is not safe, or when a file
/// or a link is where a folder of the ref must be.
pub fn put(common: &Path, wanted: &str, to: &str, was: Was<'_>) -> Result<(), ProjectError> {
    let wanted = name(wanted).ok_or(ProjectError::Escapes)?;
    let to = id(to).ok_or(ProjectError::Odd("an ID"))?;
    let common = open(common)?;
    let (above, last) = common
        .above(Path::new(wanted), Make::Missing)
        .map_err(|_| ProjectError::Escapes)?
        .ok_or(ProjectError::Escapes)?;
    replace(&above, last, &format!("{to}\n"), || {
        let now = match held(&common, wanted)? {
            Some(Held::Id(id)) => Some(id),
            Some(Held::Ref(_)) => return Err(ProjectError::Odd("a branch that names a ref")),
            None => None,
        };
        match (was, now.as_deref()) {
            (Was::Absent, None) | (Was::Any, _) => Ok(()),
            (Was::At(before), Some(now)) if before == now => Ok(()),
            _ => Err(ProjectError::Moved),
        }
    })
}

/// Points a ref of the runner at `to`, whatever is at its place. The refs below `refs/anima`
/// in the repository of the person are the runner's, but each thread of the project can
/// write there: a lock that was left, a folder at the name of the ref, or a file where a
/// folder of the ref must be would keep the runner from saving work for good. Each of them
/// goes first; no link is followed.
///
/// # Errors
/// A [`ProjectError`] for a name that is not safe, or the error of the file system.
pub fn own_put(common: &Path, wanted: &str, to: &str) -> Result<(), ProjectError> {
    let wanted = name(wanted)
        .filter(|wanted| wanted.starts_with("refs/anima/"))
        .ok_or(ProjectError::Escapes)?;
    let to = id(to).ok_or(ProjectError::Odd("an ID"))?;
    let common = open(common)?;
    let (above, last) = common
        .above(Path::new(wanted), Make::Over)?
        .ok_or(ProjectError::Escapes)?;
    let mut lock = last.to_owned();
    lock.push(".lock");
    for in_the_way in [lock.as_os_str(), last] {
        cleared(&above, in_the_way)?;
    }
    replace(&above, last, &format!("{to}\n"), || Ok(()))
}

/// Takes away what is at `name` in the open folder `above`: a file or a link is removed, a
/// folder with what it holds. A folder that cannot be removed (too deep, or one that a
/// process fills again) gets a different name, which no later write uses.
fn cleared(above: &Dir, name: &OsStr) -> Result<(), ProjectError> {
    if above.remove(name, false).is_ok() || above.remove_all(name).is_ok() {
        return Ok(());
    }
    let mut aside = OsString::from("anima-aside-");
    aside.push(uuid::Uuid::now_v7().as_simple().to_string());
    Ok(above.rename(name, &aside)?)
}

/// Removes the loose file of the ref `wanted` when it points at `at`: the undo of a [`put`]
/// of a new branch that holds no work.
///
/// # Errors
/// A [`ProjectError`] for a name that is not safe, or the error of the file system.
pub fn take_back(common: &Path, wanted: &str, at: &str) -> Result<(), ProjectError> {
    let wanted = Path::new(name(wanted).ok_or(ProjectError::Escapes)?);
    let common = open(common)?;
    let (Some(Held::Id(now)), Some((above, last))) =
        (loose(&common, wanted)?, common.above(wanted, Make::No)?)
    else {
        return Ok(());
    };
    if now == at {
        above.remove(last, false)?;
    }
    Ok(())
}

/// Puts the checkout of `layout` on `branch`.
///
/// # Errors
/// A [`ProjectError`] for a name that is not safe or a `HEAD` that git holds.
pub fn point_head(layout: &Layout, branch: &str) -> Result<(), ProjectError> {
    let to = format!("refs/heads/{branch}");
    let to = name(&to).ok_or(ProjectError::Escapes)?;
    let git_dir = layout.entry()?;
    replace(
        &git_dir,
        walk::name("HEAD"),
        &format!("ref: {to}\n"),
        || Ok(()),
    )
}

#[cfg(test)]
mod tests;
