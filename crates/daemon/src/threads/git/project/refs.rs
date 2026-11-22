//! The refs of the repository of the person as files: `HEAD`, the loose refs and `packed-refs`.
//! The runner reads them and moves a branch of a thread here with no `git`, so that no hook of
//! the repository runs for it (`reference-transaction`).
//!
//! [`id`], [`name`] and [`in_packed`] are pure; the rest reads and writes files.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::{Layout, ProjectError, limited, small};

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

/// Reads the file of a ref; `None` when there is no file (a folder is none too).
fn loose(file: &Path) -> Result<Option<Held>, ProjectError> {
    if !std::fs::symlink_metadata(file).is_ok_and(|about| !about.is_dir()) {
        return Ok(None);
    }
    let text = small(file).map_err(|_| ProjectError::Odd("a ref"))?;
    let text = text.trim_end();
    if let Some(to) = text.strip_prefix("ref: ") {
        return name(to)
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
fn held(common: &Path, wanted: &str) -> Result<Option<Held>, ProjectError> {
    if let Some(found) = loose(&common.join(wanted))? {
        return Ok(Some(found));
    }
    let Ok(packed) = limited(&common.join("packed-refs"), MOST_PACKED) else {
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
    Ok(match held(common, wanted)? {
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
    match loose(&layout.git_dir.join("HEAD"))? {
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

/// Writes `text` to `file` as git does: to `<file>.lock`, made new, then moved into place.
/// `check` runs while the lock is held. A link at the place of the file is replaced, never
/// followed.
fn replace(
    file: &Path,
    text: &str,
    check: impl FnOnce() -> Result<(), ProjectError>,
) -> Result<(), ProjectError> {
    let mut lock = file.as_os_str().to_owned();
    lock.push(".lock");
    let lock = PathBuf::from(lock);
    let mut held = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => ProjectError::Locked(lock.display().to_string()),
            _ => ProjectError::Io(error),
        })?;
    let written = check().and_then(|()| {
        held.write_all(text.as_bytes())?;
        held.sync_all()?;
        Ok(std::fs::rename(&lock, file)?)
    });
    if written.is_err() {
        // The lock is of this call alone; the first failure is the answer.
        let _ = std::fs::remove_file(&lock);
    }
    written
}

/// Points the ref `wanted` of the shared Git folder at `to`, only when it is as `was` says.
/// No reflog line is written.
///
/// # Errors
/// [`ProjectError::Moved`] when the ref is not as `was` says; [`ProjectError::Locked`] when
/// git holds the ref; [`ProjectError::Escapes`] for a name or a folder that leaves `refs`.
pub fn put(common: &Path, wanted: &str, to: &str, was: Was<'_>) -> Result<(), ProjectError> {
    let wanted = name(wanted).ok_or(ProjectError::Escapes)?;
    let to = id(to).ok_or(ProjectError::Odd("an ID"))?;
    let file = common.join(wanted);
    let folder = file.parent().ok_or(ProjectError::Escapes)?;
    std::fs::create_dir_all(folder)?;
    // A link in the path must not send the write out of the refs of this repository.
    let refs = std::fs::canonicalize(common.join("refs"))?;
    if !std::fs::canonicalize(folder)?.starts_with(&refs) {
        return Err(ProjectError::Escapes);
    }
    replace(&file, &format!("{to}\n"), || {
        let now = match held(common, wanted)? {
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

/// Removes the loose file of the ref `wanted` when it points at `at`: the undo of a [`put`]
/// of a new branch that holds no work.
///
/// # Errors
/// A [`ProjectError`] for a name that is not safe, or the error of the file system.
pub fn take_back(common: &Path, wanted: &str, at: &str) -> Result<(), ProjectError> {
    let wanted = name(wanted).ok_or(ProjectError::Escapes)?;
    let file = common.join(wanted);
    match loose(&file)? {
        Some(Held::Id(now)) if now == at => Ok(std::fs::remove_file(file)?),
        _ => Ok(()),
    }
}

/// Puts the checkout of `layout` on `branch`.
///
/// # Errors
/// A [`ProjectError`] for a name that is not safe or a `HEAD` that git holds.
pub fn point_head(layout: &Layout, branch: &str) -> Result<(), ProjectError> {
    let to = format!("refs/heads/{branch}");
    let to = name(&to).ok_or(ProjectError::Escapes)?;
    replace(
        &layout.git_dir.join("HEAD"),
        &format!("ref: {to}\n"),
        || Ok(()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";

    fn common() -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::create_dir_all(folder.path().join("refs/heads")).expect("refs");
        folder
    }

    #[test]
    fn a_name_that_is_no_ref_or_leaves_the_refs_is_refused() {
        for bad in [
            "HEAD",
            "refs/",
            "refs/../config",
            "refs/heads/a b",
            "config",
            "refs/heads/x.lock",
        ] {
            assert_eq!(name(bad), None, "{bad}");
        }
        assert_eq!(
            name("refs/heads/anima/12345678"),
            Some("refs/heads/anima/12345678")
        );
        assert_eq!(id(A), Some(A));
        assert_eq!(id("HEAD"), None);
        assert_eq!(id(&A.to_uppercase().replace('1', "A")), None);
    }

    #[test]
    fn a_loose_ref_wins_over_a_packed_one_and_a_ref_that_names_a_ref_is_followed() {
        let folder = common();
        let packed =
            format!("# pack-refs with: peeled\n{A} refs/heads/main\n^{B}\n{A} refs/tags/v1\n");
        std::fs::write(folder.path().join("packed-refs"), packed).expect("packed");
        let found = |name| resolve(folder.path(), name).expect("readable");
        assert_eq!(found("refs/heads/main").as_deref(), Some(A));
        assert_eq!(found("refs/heads/none"), None);
        std::fs::write(folder.path().join("refs/heads/main"), format!("{B}\n")).expect("loose");
        assert_eq!(found("refs/heads/main").as_deref(), Some(B));
        std::fs::create_dir_all(folder.path().join("refs/remotes/origin")).expect("a folder");
        let head = folder.path().join("refs/remotes/origin/HEAD");
        std::fs::write(head, "ref: refs/heads/main\n").expect("a ref");
        assert_eq!(found("refs/remotes/origin/HEAD").as_deref(), Some(B));
        assert_eq!(
            names(folder.path(), "refs/remotes/origin/HEAD")
                .expect("readable")
                .as_deref(),
            Some("refs/heads/main")
        );
    }

    #[test]
    fn a_branch_moves_only_from_where_it_was() {
        let folder = common();
        let put = |to, was| put(folder.path(), "refs/heads/anima/x", to, was);
        assert!(matches!(put(B, Was::At(A)), Err(ProjectError::Moved)));
        put(A, Was::Absent).expect("a new branch");
        assert!(matches!(put(B, Was::Absent), Err(ProjectError::Moved)));
        assert!(matches!(put(B, Was::At(B)), Err(ProjectError::Moved)));
        put(B, Was::At(A)).expect("a move");
        let file = folder.path().join("refs/heads/anima/x");
        assert_eq!(
            std::fs::read_to_string(&file).expect("a ref"),
            format!("{B}\n")
        );
        assert!(!folder.path().join("refs/heads/anima/x.lock").exists());
    }

    #[test]
    fn a_ref_that_git_holds_is_not_written() {
        let folder = common();
        std::fs::write(folder.path().join("refs/heads/x.lock"), "").expect("a lock");
        let held = put(folder.path(), "refs/heads/x", A, Was::Absent);
        assert!(matches!(held, Err(ProjectError::Locked(_))));
        assert!(!folder.path().join("refs/heads/x").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_refs_does_not_send_a_write_out_of_them() {
        let folder = common();
        let outside = tempfile::tempdir().expect("a folder");
        let link = folder.path().join("refs/heads/anima");
        std::os::unix::fs::symlink(outside.path(), &link).expect("a link");
        let sent = put(folder.path(), "refs/heads/anima/x", A, Was::Absent);
        assert!(matches!(sent, Err(ProjectError::Escapes)));
        assert_eq!(
            std::fs::read_dir(outside.path()).expect("a folder").count(),
            0
        );
    }
}
