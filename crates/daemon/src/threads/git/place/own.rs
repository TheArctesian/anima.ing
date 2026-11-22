//! The Git folder of the runner for one worktree (THR-050): a bare Git folder in the folder of
//! the records, outside each repository and each worktree. Each `git` of the runner for the
//! worktree runs with it, so that `git` reads no configuration, no hook and no `info` folder
//! of the repository of the person. The checkpoints and their refs are in it.
//!
//! It has its own copy of each object that the runner reads. An agent can write the object
//! store of the repository of the person, and `git` does not check the content of an object
//! against its ID when it reads one: a borrowed object could show the person one text and
//! commit a different one. So [`learn`] copies what a commit of the branch needs through
//! `git index-pack`, which names each object by the hash of its content, and each run that
//! shows or commits something reads only this folder. Only the count of commits and a push,
//! which need the whole history, borrow the store of the person ([`borrowing`]).
//!
//! I/O: `git init`, `git pack-objects`, `git index-pack`, `git reset`, and a few files.

use std::ffi::OsStr;
use std::path::Path;

use super::super::project::Layout;
use super::super::project::refs::{self, Head};
use super::super::run::{Git, GitError, Run};
use super::Worktree;

/// The ref of the last commit of the branch whose objects the runner copied.
const KNOWN: &str = "refs/anima/known";

/// Makes the Git folder of the runner at `own`. `long_ids` is for a repository whose IDs have
/// 64 digits.
///
/// # Errors
/// A [`GitError`] when `git init` fails.
pub async fn make(git: &Git, own: &Path, long_ids: bool) -> Result<(), GitError> {
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
    // The Git folder is named: `git` looks for no repository above the folder it runs in.
    let init = Run::new(above, init).with_env("GIT_DIR", own);
    git.output(init).await.map(drop)
}

/// A run that can read the object store of the repository of the person besides the one of
/// the runner. What it reads there is not checked: only for a count, a copy that is checked
/// after ([`learn`]), and a push, whose remote checks each object.
#[must_use]
pub fn borrowing<'a>(run: Run<'a>, common: &Path) -> Run<'a> {
    run.with_env("GIT_ALTERNATE_OBJECT_DIRECTORIES", common.join("objects"))
}

/// Where the worktree is, with the objects of its last commit copied and checked ([`learn`]).
///
/// # Errors
/// A [`GitError`] when the branch cannot be read or its commit is not what its ID says.
pub async fn tip(git: &Git, worktree: &Worktree) -> Result<Head, GitError> {
    let head = refs::head(&worktree.layout())?;
    if let Some(commit) = &head.commit {
        learn(git, worktree, commit).await?;
    }
    Ok(head)
}

/// Copies the commit `commit`, its trees and its files from the repository of the person into
/// the Git folder of the runner, each named by the hash of its content. Its history is not
/// copied. An object whose content is not what its ID says does not arrive under that ID, and
/// the commit is then refused.
///
/// # Errors
/// A [`GitError`] when `commit` is no commit, an object of it is missing or forged, or a step
/// of `git` fails.
pub async fn learn(git: &Git, worktree: &Worktree, commit: &str) -> Result<(), GitError> {
    let dir = worktree.folder();
    if named(git, dir, &format!("{KNOWN}^{{commit}}"))
        .await
        .as_deref()
        == Some(commit)
    {
        return Ok(());
    }
    let tree = commit_itself(git, worktree, commit).await?;
    // Its trees and files, without what the commit before it had.
    let wanted = match named(git, dir, &format!("{KNOWN}^{{tree}}")).await {
        Some(known) => format!("{tree}\n^{known}\n"),
        None => format!("{tree}\n"),
    };
    let record = worktree.record();
    lent(&record.common_dir)?;
    let incoming = fresh(&record.own, "incoming")?;
    let base = incoming.join("p");
    let pack: [&OsStr; 4] = [
        "pack-objects".as_ref(),
        "--revs".as_ref(),
        "--quiet".as_ref(),
        base.as_os_str(),
    ];
    let pack = borrowing(Run::new(dir, pack), &record.common_dir);
    git.output(pack.with_input(wanted.into_bytes())).await?;
    // The store of the person named no second store before the copy and none after it.
    self::lent(&record.common_dir)?;
    for pack in packs_in(&incoming)? {
        // Only the pack is taken; `index-pack` names each object by its content. The pack
        // comes into the store of the runner after that, with its new list of names.
        let index = pack.with_extension("idx");
        let _ = std::fs::remove_file(&index);
        let name: [&OsStr; 2] = ["index-pack".as_ref(), pack.as_os_str()];
        git.output(Run::new(dir, name)).await?;
        for file in [&pack, &index] {
            let placed = file
                .file_name()
                .map(|name| record.own.join("objects/pack").join(name));
            let placed = placed.ok_or(GitError::NotFound)?;
            std::fs::rename(file, placed).map_err(GitError::Files)?;
        }
    }
    let _ = std::fs::remove_dir_all(&incoming);
    // Each object of the commit is in the folder of the runner now, or the commit is refused.
    let whole = [
        "rev-list",
        "--objects",
        "--quiet",
        "--no-walk",
        commit,
        "--",
    ];
    let whole = git.output(Run::new(dir, whole)).await;
    whole.map_err(|_| forged())?;
    let keep = ["update-ref", "--no-deref", KNOWN, commit];
    git.output(Run::new(dir, keep)).await.map(drop)
}

/// A new empty folder `name` in the Git folder of the runner.
fn fresh(own: &Path, name: &str) -> Result<std::path::PathBuf, GitError> {
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

/// What the object store of the person borrows from: nothing, in a store that the runner
/// copies from. A store that names a second store (`info/alternates`) would let an agent
/// have the runner copy the objects of a repository that the agent cannot read.
///
/// # Errors
/// A refusal when the store is no real folder or names a second store.
fn lent(common: &Path) -> Result<(), GitError> {
    let objects = common.join("objects");
    let real = objects.symlink_metadata().is_ok_and(|about| about.is_dir());
    let names_more = ["info/alternates", "info/http-alternates"]
        .iter()
        .any(|file| objects.join(file).symlink_metadata().is_ok());
    if real && !names_more {
        Ok(())
    } else {
        Err(GitError::Failed {
            code: None,
            said:
                "the repository borrows objects from a different place (objects/info/alternates); \
                   Anima does not read such a repository"
                    .to_owned(),
        })
    }
}

/// The refusal of a commit with an object that did not arrive under its ID.
fn forged() -> GitError {
    GitError::Failed {
        code: None,
        said: "an object of the repository is missing or is not what its ID says".to_owned(),
    }
}

/// The ID that `name` resolves to in the Git folder of the runner alone.
async fn named(git: &Git, dir: &Path, name: &str) -> Option<String> {
    let verify = ["rev-parse", "--verify", "--quiet", "--end-of-options", name];
    git.text(Run::new(dir, verify)).await.ok()
}

/// Copies the commit object `commit`: read from the store of the person, written under the
/// hash of its text. Gives the ID of its tree.
async fn commit_itself(git: &Git, worktree: &Worktree, commit: &str) -> Result<String, GitError> {
    let dir = worktree.folder();
    let read = Run::new(dir, ["cat-file", "commit", commit]);
    let text = git
        .output(borrowing(read, &worktree.record().common_dir))
        .await?;
    let tree = text
        .strip_prefix(b"tree ")
        .and_then(|rest| rest.split(|byte| *byte == b'\n').next())
        .and_then(|tree| std::str::from_utf8(tree).ok())
        .and_then(refs::id)
        .ok_or_else(forged)?
        .to_owned();
    let write = ["hash-object", "-t", "commit", "-w", "--stdin"];
    if git.text(Run::new(dir, write).with_input(text)).await? != commit {
        return Err(forged());
    }
    Ok(tree)
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
            let placed = file.file_name().map(|name| packs.join(name));
            let placed = placed.ok_or(GitError::NotFound)?;
            std::fs::copy(&file, placed).map_err(GitError::Files)?;
        }
    }
    let _ = std::fs::remove_dir_all(&outgoing);
    Ok(())
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
    // No file of the worktree is read: the index only names what the commit has.
    let read = Run::new(worktree.folder(), ["read-tree", commit]);
    git.output(read.with_env("GIT_INDEX_FILE", &fresh)).await?;
    std::fs::rename(&fresh, git_dir.join("index")).map_err(GitError::Files)
}
