//! The copies of the files of one snapshot (P12, THR-050): `git` only lists which files of the
//! worktree differ; the runner copies each of them into a folder of its own with no link
//! followed and at most the bytes that the thread can still hold, and `git` reads the copies.
//!
//! I/O: copies files of the worktree into a folder of the runner.

use std::path::{Path, PathBuf};

use super::super::place::walk::Dir;
use super::super::place::{self, Worktree, own, shared};
use super::super::run::{Git, GitError, Run};
use super::super::worktree::files;

/// The copies of the listed files in `staging`, and the names to stage. When the files are
/// larger than the `left` bytes, what no checkpoint needs goes first and the copies are made
/// one more time.
pub(super) async fn fitted(
    git: &Git,
    worktree: &Worktree,
    (staging, lists): (&Path, &Lists<'_>),
    mut left: u64,
) -> Result<Staged, GitError> {
    let dir = worktree.folder();
    let mut staged = copied(dir, staging, lists, &mut left);
    if matches!(&staged, Err(full) if full.kind() == std::io::ErrorKind::FileTooLarge) {
        own::small(git, worktree).await?;
        left = own::within(git, worktree).await?;
        staged = copied(dir, staging, lists, &mut left);
    }
    staged.map_err(|failed| match failed.kind() {
        std::io::ErrorKind::FileTooLarge => GitError::Failed {
            code: None,
            said: if shared::free_for(&worktree.record().own) == 0 {
                "the disk of Anima's records is nearly full: no checkpoint and no list of \
                 changes can be made until there is room on it"
                    .to_owned()
            } else {
                failed.to_string()
            },
        },
        // These words name the file or the folder and what the person can do.
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidData => {
            GitError::Failed {
                code: None,
                said: failed.to_string(),
            }
        }
        _ => GitError::Files(failed),
    })
}

/// The names to stage for one snapshot, each with a zero byte after it.
#[derive(Default)]
pub(super) struct Staged {
    /// The files that have a copy.
    pub(super) there: Vec<u8>,
    /// The files of the last commit that are gone. They are staged apart and first: `git add`
    /// refuses the name of a file below a folder that is now a link.
    pub(super) gone: Vec<u8>,
}

/// The error of a file that the runner cannot read, with its name and what to do.
fn unreadable(path: &Path, failed: std::io::Error) -> std::io::Error {
    use std::io::ErrorKind as Kind;
    match failed.kind() {
        Kind::PermissionDenied => {}
        // These have their own words already.
        Kind::FileTooLarge | Kind::InvalidData => return failed,
        // Each other failure of one file is named too, so that the person can act on it.
        _ => {
            let said = format!(
                "the file {} of the worktree cannot be saved ({failed}); delete or move it",
                files::shown(path)
            );
            return std::io::Error::new(Kind::InvalidData, said);
        }
    }
    std::io::Error::new(
        failed.kind(),
        format!(
            "the file {} of the worktree, or a folder above it, cannot be read; make it \
             readable or delete it",
            files::shown(path)
        ),
    )
}

/// What `git` listed for one snapshot.
pub(super) struct Lists<'a> {
    /// The files of the last commit that differ or are gone.
    pub(super) changed: &'a [u8],
    /// The files that the last commit does not have.
    pub(super) new: &'a [u8],
    /// The files of rules, which are only there for `git` to read.
    pub(super) rules: &'a [u8],
    /// The nested repositories that the last commit names.
    pub(super) nested: &'a [String],
    /// For a removal: each file of a repository inside the worktree, and where it is saved
    /// ([`inner_files`]). Empty for a checkpoint.
    pub(super) inner: &'a [(PathBuf, PathBuf)],
}

/// Copies each listed file of the worktree at `dir` into the new folder `staging`, with no
/// link followed and at most `left` bytes in all, and gives the names to stage. A changed
/// file that is gone is staged too, as a removal. The copies are written through the open
/// folder `staging`, each made new: no copy is written through a link that an earlier copy
/// put there.
fn copied(
    dir: &Path,
    staging: &Path,
    lists: &Lists<'_>,
    left: &mut u64,
) -> std::io::Result<Staged> {
    let _ = std::fs::remove_dir_all(staging);
    place::private_folder(staging)?;
    let root = files::Root::open(dir)?;
    let into = Dir::open(staging)?;
    let ours = |name: &&[u8]| {
        let below = |nested: &String| {
            name.strip_prefix(nested.as_bytes())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(b"/"))
        };
        // A name that ends with `/` is a repository inside the worktree that nothing names.
        // The folder of a copy of the history that was cancelled is no file of the thread.
        let of_a_copy = name.starts_with(shared::pack::PREFIX.as_bytes());
        !name.is_empty() && !name.ends_with(b"/") && !of_a_copy && !lists.nested.iter().any(below)
    };
    let mut staged = Staged::default();
    let mut taken = std::collections::HashSet::new();
    for (list, tracked, stage) in [
        (lists.changed, true, true),
        (lists.new, false, true),
        (lists.rules, false, false),
    ] {
        for name in list.split(|byte| *byte == 0).filter(ours) {
            let path = files::path(name);
            // A file of rules that changed is in two lists: it is copied one time.
            if !files::inside(&path) || !taken.insert(name) {
                continue;
            }
            let there = root
                .take(&path, &into, left)
                .map_err(|failed| unreadable(&path, failed))?;
            let names = match (stage, there, tracked) {
                (true, true, _) => &mut staged.there,
                (true, false, true) => &mut staged.gone,
                _ => continue,
            };
            names.extend_from_slice(name);
            names.push(0);
        }
    }
    for (file, kept_at) in lists.inner {
        let there = root
            .take_to(file, (&into, kept_at), left)
            .map_err(|failed| unreadable(file, failed))?;
        if there {
            let name = kept_at.as_os_str().as_encoded_bytes();
            staged.there.extend_from_slice(name);
            staged.there.push(0);
        }
    }
    Ok(staged)
}

/// The most repositories inside a worktree whose files one removal saves.
const MOST_INNER: usize = 64;

/// The files of each repository inside the worktree, for a removal: where each is, and where
/// it is saved. A checkpoint leaves such a repository alone; a removal deletes it, so it saves
/// its files first, as plain files. The Git folder of the repository (its own history) is
/// not saved.
///
/// - A repository that nothing names (a `git init` or a clone of an agent, in `new` with `/`
///   after its name) is saved at its place.
/// - A repository that the last commit names (`nested`) keeps its entry; its files are saved
///   below one folder with a name that nothing in the worktree can have, `kept`.
///
/// `git` lists the files of each with the rules of that repository for what is ignored, as
/// for the worktree itself: what is built there (`node_modules`, `target`) is not saved.
///
/// # Errors
/// A [`GitError`] when a listing fails or there are more than [`MOST_INNER`] repositories.
pub(super) async fn inner_files(
    git: &Git,
    worktree: &Worktree,
    (new, nested): (&[u8], &[String]),
) -> Result<Vec<(PathBuf, PathBuf)>, GitError> {
    let dir = worktree.folder();
    let kept = PathBuf::from(format!("anima-kept-{}", uuid::Uuid::now_v7().as_simple()));
    let unnamed = new
        .split(|byte| *byte == 0)
        .filter_map(|name| name.strip_suffix(b"/"))
        .map(|name| (files::path(name), files::path(name)));
    let named = nested
        .iter()
        .map(|name| (PathBuf::from(name), kept.join(name)));
    let folders: Vec<(PathBuf, PathBuf)> = unnamed
        .chain(named)
        .filter(|(folder, _)| files::inside(folder))
        .collect();
    if folders.len() > MOST_INNER {
        return Err(GitError::Failed {
            code: None,
            said: "the worktree holds too many repositories of its own to save them; delete or \
                   move some of those folders, then remove the worktree again"
                .to_owned(),
        });
    }
    let root = files::Root::open(dir).map_err(GitError::Files)?;
    // An index that is not there: each file of the repository is new to this listing.
    let none = worktree.record().own.join("index.none");
    let _ = std::fs::remove_file(&none);
    let mut found = Vec::new();
    for (folder, saved_at) in folders {
        let at = dir.join(&folder);
        if !at.is_dir() {
            continue;
        }
        let list = Run::new(&at, NEW_FILES)
            .with_env("GIT_WORK_TREE", &at)
            .with_env("GIT_INDEX_FILE", &none);
        let listed = git.output(list).await.map_err(|_| unlisted(&folder))?;
        for name in listed
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let inside = files::path(name.strip_suffix(b"/").unwrap_or(name));
            if !files::inside(&inside) {
                continue;
            }
            // A repository deeper down is listed as one name: its files are walked.
            let deeper = if name.ends_with(b"/") {
                root.files_below(&folder.join(&inside))
                    .map_err(GitError::Files)?
            } else {
                vec![folder.join(&inside)]
            };
            for file in deeper {
                let rest = file.strip_prefix(&folder).unwrap_or(&file).to_owned();
                found.push((file, saved_at.join(rest)));
            }
        }
    }
    Ok(found)
}

/// The refusal for a repository inside the worktree whose files `git` cannot list.
fn unlisted(folder: &Path) -> GitError {
    GitError::Failed {
        code: None,
        said: format!(
            "the files of the repository in the folder {} of the worktree cannot be listed; \
             delete or move that folder, then remove the worktree again",
            files::shown(folder)
        ),
    }
}

/// Lists the files of a work folder that are in no index and that git does not ignore.
const NEW_FILES: [&str; 4] = ["ls-files", "-z", "--others", "--exclude-standard"];

/// The folder of the copies of one snapshot; it is removed when the value is dropped.
pub(super) struct Staging(pub(super) PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
