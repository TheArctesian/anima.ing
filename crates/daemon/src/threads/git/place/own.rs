//! The Git folder of the runner for one worktree (THR-050): a bare Git folder in the folder of
//! the records, outside each repository and each worktree. Each `git` of the runner for the
//! worktree runs with it, so that `git` reads no configuration, no hook and no `info` folder
//! of the repository of the person. It borrows the objects of that repository through
//! `objects/info/alternates`; the checkpoints and their refs are in it.
//!
//! I/O: `git init`, `git pack-objects`, `git reset`, and a few files.

use std::ffi::OsStr;
use std::path::Path;

use super::super::project::Layout;
use super::super::run::{Git, GitError, Run};
use super::Worktree;

/// Makes the Git folder of the runner at `own`, with the objects of `common` to read.
/// `long_ids` is for a repository whose IDs have 64 digits.
///
/// # Errors
/// A [`GitError`] when `git init` fails or a file cannot be written.
pub async fn make(git: &Git, own: &Path, common: &Path, long_ids: bool) -> Result<(), GitError> {
    let above = own.parent().ok_or(GitError::NotFound)?;
    super::private_folder(above).map_err(GitError::Files)?;
    // A folder that a call left half made holds nothing that a thread has.
    let _ = std::fs::remove_dir_all(own);
    let format = if long_ids { "sha256" } else { "sha1" };
    let format = format!("--object-format={format}");
    let init: [&OsStr; 7] = [
        "init".as_ref(),
        "--quiet".as_ref(),
        "--bare".as_ref(),
        // No template: no sample hook and no file of the person comes into the folder.
        "--template=".as_ref(),
        format.as_ref(),
        "--".as_ref(),
        own.as_os_str(),
    ];
    git.output(Run::new(above, init)).await?;
    let objects = common.join("objects");
    let borrowed = format!("{}\n", objects.display());
    std::fs::write(own.join("objects/info/alternates"), borrowed).map_err(GitError::Files)
}

/// Copies to the repository of the person each object of `commit` that it does not have,
/// as one pack file. `known` is a commit that it has: nothing of it is copied.
///
/// # Errors
/// A [`GitError`] when the pack cannot be written.
pub async fn give(
    git: &Git,
    worktree: &Worktree,
    commit: &str,
    known: Option<&str>,
) -> Result<(), GitError> {
    let common = &worktree.record().common_dir;
    let packs = common.join("objects/pack");
    // A link at the place of the packs must not send the file out of the repository.
    if std::fs::canonicalize(&packs).ok().as_deref() != Some(packs.as_path()) {
        return Err(GitError::Replaced);
    }
    let wanted = match known {
        Some(known) => format!("{commit}\n^{known}\n"),
        None => format!("{commit}\n"),
    };
    let named = packs.join("pack");
    let pack: [&OsStr; 4] = [
        "pack-objects".as_ref(),
        "--revs".as_ref(),
        "--quiet".as_ref(),
        named.as_os_str(),
    ];
    let run = Run::new(worktree.folder(), pack);
    git.output(run.with_input(wanted.into_bytes()))
        .await
        .map(drop)
}

/// Makes the index of the worktree, the one that the `git` of the agent and of the person
/// reads, equal to `commit`: nothing is staged. The index is written new and moved into
/// place; the one that was there is not read.
///
/// # Errors
/// A [`GitError`] when the index cannot be written.
pub async fn follow(git: &Git, worktree: &Worktree, commit: &str) -> Result<(), GitError> {
    let Layout { git_dir, .. } = worktree.layout();
    let fresh = git_dir.join("index.anima");
    // A file of an earlier call that did not end; `git` makes the new one with a lock.
    let _ = std::fs::remove_file(&fresh);
    let reset = Run::new(worktree.folder(), ["reset", "--quiet", commit, "--"]);
    git.output(reset.with_env("GIT_INDEX_FILE", &fresh)).await?;
    std::fs::rename(&fresh, git_dir.join("index")).map_err(GitError::Files)
}
