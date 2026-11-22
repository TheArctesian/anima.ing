//! The Git folder of the runner for one worktree (THR-050): a bare Git folder in the folder of
//! the records, outside each repository and each worktree. Each `git` of the runner for the
//! worktree runs with it, so that `git` reads no configuration, no hook and no `info` folder
//! of the repository of the person. The checkpoints and their refs are in it.
//!
//! It reads no object of the repository of the person either: the history of the project is
//! in a checked copy of the runner ([`super::shared`]), which this folder borrows from.
//!
//! I/O: `git init`, `git pack-objects`, `git read-tree`, and a few files.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::super::project::refs::{self, Head};
use super::super::run::{Git, GitError, Run};
use super::Worktree;
use super::shared;
use super::walk::{self, Dir, Make};

/// Makes a bare Git folder of the runner at `folder`, with no template: no sample hook and no
/// file of the person comes into it.
///
/// # Errors
/// A [`GitError`] when `git init` fails.
pub(super) async fn init(git: &Git, folder: &Path, long_ids: bool) -> Result<(), GitError> {
    let above = folder.parent().ok_or(GitError::NotFound)?;
    super::private_folder(above).map_err(GitError::Files)?;
    // A folder that a call left half made holds nothing that a thread has.
    let _ = std::fs::remove_dir_all(folder);
    let format = if long_ids { "sha256" } else { "sha1" };
    let format = format!("--object-format={format}");
    let init: [&OsStr; 7] = [
        "init".as_ref(),
        "--quiet".as_ref(),
        "--bare".as_ref(),
        "--template=".as_ref(),
        format.as_ref(),
        "--".as_ref(),
        folder.as_os_str(),
    ];
    // The Git folder is named: `git` looks for no repository above the folder it runs in.
    let init = Run::new(above, init).with_env("GIT_DIR", folder);
    git.output(init).await.map(drop)
}

/// Makes the Git folder of the runner at `own`, which borrows the objects of the checked copy
/// of its project at `shared`. `long_ids` is for a repository whose IDs have 64 digits.
///
/// # Errors
/// A [`GitError`] when `git init` fails or the file cannot be written.
pub async fn make(git: &Git, own: &Path, shared: &Path, long_ids: bool) -> Result<(), GitError> {
    init(git, own, long_ids).await?;
    // The copy is a folder of the runner: its path is not text that an agent wrote.
    let borrowed = format!("{}\n", shared.join("objects").display());
    std::fs::write(own.join("objects/info/alternates"), borrowed).map_err(GitError::Files)
}

/// Where the worktree is, with the history of its last commit in the checked copy of the
/// project ([`shared::learn`]).
///
/// # Errors
/// A [`GitError`] when the branch cannot be read or its commit is not what its ID says.
pub async fn tip(git: &Git, worktree: &Worktree) -> Result<Head, GitError> {
    let head = refs::head(&worktree.layout())?;
    if let Some(commit) = &head.commit {
        shared::learn(git, worktree, &[commit]).await?;
    }
    Ok(head)
}

/// A new empty folder `name` in a Git folder of the runner.
pub(super) fn fresh(own: &Path, name: &str) -> Result<std::path::PathBuf, GitError> {
    let folder = own.join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir(&folder).map_err(GitError::Files)?;
    Ok(folder)
}

/// The pack files in `folder`.
fn packs_in(folder: &Path) -> Result<Vec<std::path::PathBuf>, GitError> {
    let listed = std::fs::read_dir(folder).map_err(GitError::Files)?;
    let is_pack = |path: &std::path::PathBuf| path.extension().is_some_and(|kind| kind == "pack");
    Ok(listed
        .flatten()
        .map(|file| file.path())
        .filter(is_pack)
        .collect())
}

/// Copies to the repository of the person each object of `commit` that it does not have,
/// as one pack file, from the Git folder of the runner alone. `known` is a commit that the
/// repository has: nothing of it is copied.
///
/// # Errors
/// A [`GitError`] when the pack cannot be written.
pub async fn give(
    git: &Git,
    worktree: &Worktree,
    commit: &str,
    known: Option<&str>,
) -> Result<(), GitError> {
    // The folder of the packs is found from the open Git folder: a link in its place or in
    // the place of `objects` does not send a file out of the repository.
    let common = Dir::open(&worktree.record().common_dir).map_err(GitError::Files)?;
    let packs = common.below(Path::new("objects/pack"), Make::No);
    let packs = packs.map_err(GitError::Files)?.ok_or(GitError::Replaced)?;
    let wanted = match known {
        Some(known) => format!("{commit}\n^{known}\n"),
        None => format!("{commit}\n"),
    };
    // The pack is made in the folder of the runner and copied: the two can be on two disks.
    let outgoing = fresh(&worktree.record().own, "outgoing")?;
    let named = outgoing.join("pack");
    let pack: [&OsStr; 4] = [
        "pack-objects".as_ref(),
        "--revs".as_ref(),
        "--quiet".as_ref(),
        named.as_os_str(),
    ];
    let run = Run::new(worktree.folder(), pack);
    git.output(run.with_input(wanted.into_bytes())).await?;
    for pack in packs_in(&outgoing)? {
        // The pack first: git reads a pack only when its list of names is beside it.
        for file in [pack.clone(), pack.with_extension("idx")] {
            let name = file.file_name().ok_or(GitError::NotFound)?;
            placed(&packs, name, &file).map_err(GitError::Files)?;
        }
    }
    let _ = std::fs::remove_dir_all(&outgoing);
    Ok(())
}

/// Copies the file `from` of the runner into the open folder `into` as `name`: written under
/// a different name first, then moved into place, so no half file has the name.
fn placed(into: &Dir, name: &OsStr, from: &Path) -> std::io::Result<()> {
    let mut partial = OsString::from("anima-");
    partial.push(name);
    into.remove(&partial, false)?;
    let mut written = into.new_file(&partial, false)?;
    std::io::copy(&mut std::fs::File::open(from)?, &mut written)?;
    written.sync_all()?;
    into.rename(&partial, name)
}

/// Makes the index of the worktree, the one that the `git` of the agent and of the person
/// reads, equal to `commit`: nothing is staged. The index is written new and moved into
/// place; the one that was there is not read.
///
/// # Errors
/// A [`GitError`] when the index cannot be written.
pub async fn follow(git: &Git, worktree: &Worktree, commit: &str) -> Result<(), GitError> {
    // The index is made in the folder of the runner; no file of the worktree is read for it:
    // it only names what the commit has. It comes into the Git folder of the worktree as a
    // file, found from the open folder.
    let fresh = worktree.record().own.join("index.worktree");
    let _ = std::fs::remove_file(&fresh);
    let read = Run::new(worktree.folder(), ["read-tree", commit]);
    git.output(read.with_env("GIT_INDEX_FILE", &fresh)).await?;
    let git_dir = Dir::open(&worktree.record().git_dir).map_err(GitError::Files)?;
    placed(&git_dir, walk::name("index"), &fresh).map_err(GitError::Files)
}
