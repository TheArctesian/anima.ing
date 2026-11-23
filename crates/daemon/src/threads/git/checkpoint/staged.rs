//! The copies of the files of one snapshot (P12, THR-050): `git` only lists which files of the
//! worktree differ; the runner copies each of them into a folder of its own with no link
//! followed and at most the bytes that the thread can still hold, and `git` reads the copies.
//!
//! I/O: copies files of the worktree into a folder of the runner.

use super::super::place::{self, Worktree, own, shared};
use super::super::run::{Git, GitError};
use super::super::worktree::files;

/// The copies of the listed files in `staging`, and the names to stage. When the files are
/// larger than the `left` bytes, what no checkpoint needs goes first and the copies are made
/// one more time.
pub(super) async fn fitted(
    git: &Git,
    worktree: &Worktree,
    (staging, lists): (&std::path::Path, &Lists<'_>),
    mut left: u64,
) -> Result<Vec<u8>, GitError> {
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
        _ => GitError::Files(failed),
    })
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
    /// A removal: the files of each repository inside the worktree are taken too. A
    /// checkpoint leaves them alone; a removal deletes them, so it saves them first.
    pub(super) whole: bool,
}

/// Copies each listed file of the worktree at `dir` into the new folder `staging`, with no
/// link followed and at most `left` bytes in all, and gives the names to stage. A changed
/// file that is gone is staged too, as a removal.
fn copied(
    dir: &std::path::Path,
    staging: &std::path::Path,
    lists: &Lists<'_>,
    left: &mut u64,
) -> std::io::Result<Vec<u8>> {
    let _ = std::fs::remove_dir_all(staging);
    place::private_folder(staging)?;
    let root = files::Root::open(dir)?;
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
    let mut staged = Vec::new();
    for (list, tracked, stage) in [
        (lists.changed, true, true),
        (lists.new, false, true),
        (lists.rules, false, false),
    ] {
        for name in list.split(|byte| *byte == 0).filter(ours) {
            let path = files::path(name);
            if !files::inside(&path) {
                continue;
            }
            let there = root.take(&path, staging, left)?;
            if stage && (there || tracked) {
                staged.extend_from_slice(name);
                staged.push(0);
            }
        }
    }
    if lists.whole {
        inner(&root, staging, lists, (&mut staged, left))?;
    }
    Ok(staged)
}

/// Copies the files of each repository inside the worktree for a removal. One that nothing
/// names (a `git init` or a clone of an agent) is saved as plain files at its place. One
/// that the last commit names keeps its entry, and its files are saved beside it, in a
/// folder with `.files` after its name.
fn inner(
    root: &files::Root,
    staging: &std::path::Path,
    lists: &Lists<'_>,
    (staged, left): (&mut Vec<u8>, &mut u64),
) -> std::io::Result<()> {
    let unnamed = lists
        .new
        .split(|byte| *byte == 0)
        .filter_map(|name| name.strip_suffix(b"/"))
        .map(|name| (files::path(name), files::path(name)));
    let named = lists.nested.iter().map(|name| {
        let beside = format!("{name}.files");
        (
            std::path::PathBuf::from(name),
            std::path::PathBuf::from(beside),
        )
    });
    for (folder, saved_at) in unnamed
        .chain(named)
        .filter(|(folder, _)| files::inside(folder))
    {
        for file in root.files_below(&folder)? {
            let Ok(rest) = file.strip_prefix(&folder) else {
                continue;
            };
            let kept_at = saved_at.join(rest);
            let copy = staging.join(&kept_at);
            if let Some(above) = copy.parent() {
                std::fs::create_dir_all(above)?;
            }
            if root.take_to(&file, &copy, left)? {
                staged.extend_from_slice(kept_at.as_os_str().as_encoded_bytes());
                staged.push(0);
            }
        }
    }
    Ok(())
}

/// The folder of the copies of one snapshot; it is removed when the value is dropped.
pub(super) struct Staging(pub(super) std::path::PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
