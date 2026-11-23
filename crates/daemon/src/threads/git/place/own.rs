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

/// The bytes that the Git folder of the runner for one worktree can hold, when the person set
/// no other size: its checkpoints, and each file that changed in each of them.
pub const ROOM: u64 = 2 * 1024 * 1024 * 1024;

/// The file in the Git folder of the runner that says that the work of the worktree is saved
/// and its removal started.
pub const SAVED: &str = "anima-saved";

/// The file that says how large the folder was after the last time it was made small. While
/// it has that size, making it small again gives nothing.
const SMALL_AT: &str = "anima-small-at";

/// The bytes that a snapshot can still write into the Git folder of the runner for
/// `worktree`. Each status and each list of changes writes the changed files as objects, and
/// the bytes are the ones that the agent chose: without a size, one thread fills the disk of
/// the records for each project. Only the objects count: what a call left in a work folder
/// goes here first. A folder that is over is made small before: the objects that no
/// checkpoint, no reviewed snapshot and no kept ref of the thread needs go. The caller holds
/// the lock of the index of the worktree, so no snapshot of this worktree is in the middle of
/// its work. The disk of the records keeps free what [`shared::free_for`] says.
///
/// # Errors
/// A [`GitError`] when the folder is over its size also after that; the words say what the
/// person can do.
pub async fn within(git: &Git, worktree: &Worktree) -> Result<u64, GitError> {
    let own = &worktree.record().own;
    // The work of a worktree whose removal started is saved as it was then: no snapshot is
    // made of what is left of its files.
    if own.join(SAVED).exists() && git.own_room() != u64::MAX {
        return Err(GitError::Failed {
            code: None,
            said: "the removal of the worktree of this chat started and did not end. Its \
                   files were saved when it started; remove the worktree of the chat again, \
                   and what changed since is saved apart"
                .to_owned(),
        });
    }
    for left_over in ["staging", "restore", "outgoing"] {
        let _ = std::fs::remove_dir_all(own.join(left_over));
    }
    // A `git` that was ended in the middle leaves its lock. The caller holds the lock of
    // this worktree, so no `git` of the runner works in this folder now.
    for lock in ["index.lock", "shallow.lock"] {
        let _ = std::fs::remove_file(own.join(lock));
    }
    let objects = own.join("objects");
    let room = git.own_room();
    let mut used = super::disk::bytes(&objects);
    if used >= room {
        let at = std::fs::read_to_string(own.join(SMALL_AT)).ok();
        if at.and_then(|at| at.trim().parse::<u64>().ok()) != Some(used) {
            emptied(git, worktree).await?;
            used = super::disk::bytes(&objects);
        }
    }
    let mib = |bytes: u64| bytes / (1024 * 1024);
    // A removal saves the work also when the disk is nearly full: it frees the disk after.
    let free = if room == u64::MAX {
        shared::free_at(own)
    } else {
        shared::free_for(own)
    };
    match room.saturating_sub(used) {
        // Not the thread but the disk has no room: a snapshot with no changed file works.
        left if left > 0 => Ok(left.min(free)),
        _ => Err(GitError::Failed {
            code: None,
            said: format!(
                "the checkpoints of this chat use {} MiB of the disk of Anima's records; one \
                 chat can have {} MiB, and the disk must keep some room. Remove old \
                 checkpoints of the chat, or remove the worktree of the chat",
                mib(used),
                mib(room)
            ),
        }),
    }
}

/// [`small`], after the index of the runner was emptied: it holds the files of the last
/// snapshot, which are no checkpoint. Only for a moment when no snapshot is being made: a
/// snapshot fills the index again before it uses it.
///
/// # Errors
/// As [`small`].
pub async fn emptied(git: &Git, worktree: &Worktree) -> Result<(), GitError> {
    let empty = Run::new(worktree.folder(), ["read-tree", "--empty"]);
    git.output(empty).await?;
    small(git, worktree).await
}

/// Removes from the Git folder of the runner for `worktree` each object that no ref of it and
/// no file of its index needs, and writes down the size after it.
///
/// `git prune` also drops from the list of shallow ends each commit that no ref of the folder
/// reaches. That list is of the project, not of the refs of one thread: it is put back as
/// the copy of the project has it before `git` reads it and after `git` changed it.
///
/// # Errors
/// A [`GitError`] when `git prune` fails.
pub async fn small(git: &Git, worktree: &Worktree) -> Result<(), GitError> {
    let own = &worktree.record().own;
    shared::ends(worktree)?;
    let prune = Run::new(worktree.folder(), ["prune", "--expire=now"]).with_time(shared::LONG);
    let pruned = git.output(prune).await;
    shared::ends(worktree)?;
    pruned?;
    let used = super::disk::bytes(&own.join("objects")).to_string();
    std::fs::write(own.join(SMALL_AT), used).map_err(GitError::Files)
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
    let run = Run::new(worktree.folder(), pack).with_time(shared::LONG);
    git.output(run.with_input(wanted.into_bytes())).await?;
    for pack in packs_in(&outgoing)? {
        // The pack first: git reads a pack only when its list of names is beside it.
        for file in [pack.clone(), pack.with_extension("idx")] {
            let name = file.file_name().ok_or(GitError::NotFound)?;
            placed(&packs, name, &file).map_err(GitError::Files)?;
        }
    }
    let _ = std::fs::remove_dir_all(&outgoing);
    large_files(&common, &worktree.record().own).map_err(GitError::Files)
}

/// Copies the contents that Git LFS keeps for the runner's folder at `own` into the store of
/// the repository of the person. When the person uses Git LFS for all their repositories, a
/// snapshot holds only a short pointer for each such file and the content is beside the Git
/// folder (`lfs/objects`). A commit or a saved worktree without that content would name
/// files that are nowhere. Each content is a file named by its hash; one that is there stays.
fn large_files(common: &Dir, own: &Path) -> std::io::Result<()> {
    let mut folders = vec![(
        own.join("lfs/objects"),
        std::path::PathBuf::from("lfs/objects"),
    )];
    while let Some((from, to)) = folders.pop() {
        let Ok(listed) = std::fs::read_dir(&from) else {
            continue;
        };
        for entry in listed.flatten() {
            let (path, kind) = (entry.path(), entry.file_type()?);
            let at = to.join(entry.file_name());
            if kind.is_dir() {
                folders.push((path, at));
            } else if kind.is_file() {
                let Some((into, name)) = common.above(&at, Make::Missing)? else {
                    continue;
                };
                // A content with its whole length is there already. A shorter one is what
                // a copy that was stopped left: it is written again.
                let whole = match into.file(name)? {
                    Some(there) => there.metadata()?.len() == entry.metadata()?.len(),
                    None => false,
                };
                if !whole {
                    placed(&into, name, &path)?;
                }
            }
        }
    }
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
    let git_dir = worktree.layout().entry().map_err(GitError::Files)?;
    placed(&git_dir, walk::name("index"), &fresh).map_err(GitError::Files)
}
