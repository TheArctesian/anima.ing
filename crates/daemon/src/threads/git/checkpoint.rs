//! Checkpoints (TH42): the files of a worktree at the end of a turn, kept as a commit under a
//! private ref in the Git folder of the runner. The index and the branch of the worktree do
//! not move, and a file that git ignores is not taken (THR-050).
//!
//! I/O: runs `git` with the Git folder and the index of the runner.

use agent_threads::git::Checkpoint;
use uuid::Uuid;

use super::place::{Worktree, own, shared};
use super::run::{Git, GitError, Run};
mod staged;

use staged::{Lists, Staging, fitted};

/// Who a checkpoint commit says it is from: never the person.
const WHO: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Anima"),
    ("GIT_AUTHOR_EMAIL", "checkpoint@anima.invalid"),
    ("GIT_COMMITTER_NAME", "Anima"),
    ("GIT_COMMITTER_EMAIL", "checkpoint@anima.invalid"),
];

/// The folder of the refs of one thread.
fn refs_of(thread: Uuid) -> String {
    format!("refs/anima/checkpoints/{}", thread.as_hyphenated())
}

/// The ref of the checkpoint of `turn`; turn 0 is the start of the thread.
#[must_use]
pub fn turn_ref(thread: Uuid, turn: u32) -> String {
    format!("{}/turn/{turn}", refs_of(thread))
}

/// The ref of the files as they were before the last revert, so that it can be undone (THR-099).
#[must_use]
pub fn before_revert_ref(thread: Uuid) -> String {
    format!("{}/before-revert", refs_of(thread))
}

/// The ref of the snapshot of the uncommitted files that the person last looked at: the only
/// one that a commit takes, and kept from the clean-up of git.
#[must_use]
pub fn reviewed_ref(thread: Uuid) -> String {
    format!("{}/reviewed", refs_of(thread))
}

/// The ref of the commit of `branch` that the last push sent.
#[must_use]
pub fn pushed_ref(branch: &str) -> String {
    format!("refs/anima/pushed/{branch}")
}

/// The ref in the repository of the person that keeps the work of a worktree that was
/// removed (THR-097).
#[must_use]
pub fn kept_ref(thread: Uuid) -> String {
    format!("refs/anima/kept/{}", thread.as_hyphenated())
}

/// Writes the files of the worktree as they are now as a tree and gives its ID. `tip` is
/// the last commit of the branch. The index is the one of the runner, which no agent writes:
/// a file that the index of the worktree marks as unchanged is taken all the same.
///
/// `git` only lists which files differ. Each of them is copied to a folder of the runner with
/// no link followed ([`files`]), and `git` reads the copy: a link that an agent puts in the
/// place of a file at the right moment leads the runner to no file outside the worktree.
///
/// # Errors
/// A [`GitError`] when a step of `git` or a copy fails.
pub async fn tree_now(
    git: &Git,
    worktree: &Worktree,
    tip: Option<&str>,
) -> Result<String, GitError> {
    let dir = worktree.folder();
    let left = own::within(git, worktree).await?;
    let start = match tip {
        Some(tip) => vec!["read-tree", "--reset", tip],
        None => vec!["read-tree", "--empty"],
    };
    git.output(Run::new(dir, start)).await?;
    // Nothing is read from a nested repository that the last commit names: such an entry
    // stays as the last commit has it (THR-050).
    let nested = nested(git, Run::new(dir, LIST)).await?;
    let changed = git.output(Run::new(dir, CHANGED)).await?;
    let new = git.output(Run::new(dir, NEW)).await?;
    let rules = git.output(Run::new(dir, RULES)).await?;
    // The copies go when this call ends, on each way out.
    let held = Staging(worktree.record().own.join("staging"));
    // A removal saves the files of each repository inside the worktree too.
    let inner = if git.removing() {
        staged::inner_files(git, worktree, (&new, &nested)).await?
    } else {
        Vec::new()
    };
    let lists = Lists {
        changed: &changed,
        new: &new,
        rules: &rules,
        nested: &nested,
        inner: &inner,
    };
    let staged = fitted(git, worktree, (&held.0, &lists), left).await?;
    if !staged.is_empty() {
        // Large files take long to pack; a `git` that is ended in the middle helps nobody.
        let add = Run::new(&held.0, ADD)
            .with_env("GIT_WORK_TREE", &held.0)
            .with_time(shared::LONG);
        git.output(add.with_input(staged)).await?;
    }
    drop(held);
    git.text(Run::new(dir, ["write-tree"])).await
}

/// Lists the files of the last commit that differ in the worktree or are gone.
const CHANGED: [&str; 3] = ["ls-files", "-z", "--modified"];

/// Lists the files that the last commit does not have and that git does not ignore.
const NEW: [&str; 4] = ["ls-files", "-z", "--others", "--exclude-standard"];

/// Lists each file of rules for the files beside it: filters and line ends.
const RULES: [&str; 7] = [
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    ":(glob)**/.gitattributes",
];

/// Stages the named files of the folder of the runner; a rule of that folder that would
/// ignore one does not apply, the list is already without the ignored files.
const ADD: [&str; 6] = [
    "--literal-pathspecs",
    "add",
    "--all",
    "--force",
    "--pathspec-from-file=-",
    "--pathspec-file-nul",
];

/// Makes a commit of the files of the worktree as they are now, with no ref and no change of
/// the branch, and gives its ID. Its parent is the last commit of the branch.
///
/// # Errors
/// A [`GitError`] when the branch cannot be read or a step of `git` fails.
pub async fn snapshot(git: &Git, worktree: &Worktree) -> Result<String, GitError> {
    let tip = own::tip(git, worktree).await?.commit;
    snapshot_on(git, worktree, tip).await
}

/// As [`snapshot`], on `tip`; with no tip the commit holds each file and has no parent.
///
/// # Errors
/// A [`GitError`] when a step of `git` fails.
pub async fn snapshot_on(
    git: &Git,
    worktree: &Worktree,
    tip: Option<String>,
) -> Result<String, GitError> {
    let tree = tree_now(git, worktree, tip.as_deref()).await?;
    let mut commit = vec!["commit-tree".to_owned(), tree];
    if let Some(tip) = tip {
        commit.extend(["-p".to_owned(), tip]);
    }
    commit.extend(["-m".to_owned(), "anima checkpoint".to_owned()]);
    let mut run = Run::new(worktree.folder(), commit);
    for (name, value) in WHO {
        run = run.with_env(name, value);
    }
    git.text(run).await
}

/// The entries of an index with their modes.
const LIST: [&str; 3] = ["ls-files", "--stage", "-z"];

/// The mode of an entry that is a nested repository.
const NESTED: &str = "160000";

/// The most nested repositories one snapshot leaves out; more is refused.
const MOST_NESTED: usize = 512;

/// The paths of the nested repositories that the index of `list` names.
async fn nested(git: &Git, list: Run<'_>) -> Result<Vec<String>, GitError> {
    let listed = git.output(list).await?;
    let found: Vec<&[u8]> = listed
        .split(|byte| *byte == 0)
        .filter(|entry| entry.starts_with(NESTED.as_bytes()))
        .filter_map(|entry| entry.splitn(2, |byte| *byte == b'\t').nth(1))
        .collect();
    let paths: Option<Vec<String>> = found
        .iter()
        .map(|path| std::str::from_utf8(path).ok().map(str::to_owned))
        .collect();
    // A path that is not text cannot be left out by name, so no snapshot is made.
    paths
        .filter(|paths| paths.len() <= MOST_NESTED)
        .ok_or_else(|| GitError::Failed {
            code: None,
            said: "the worktree has a nested repository that cannot be left out".to_owned(),
        })
}

/// Points `name` at `commit`.
///
/// # Errors
/// A [`GitError`] when `git update-ref` fails.
pub async fn keep(
    git: &Git,
    worktree: &Worktree,
    name: &str,
    commit: &str,
) -> Result<(), GitError> {
    git.output(Run::new(
        worktree.folder(),
        ["update-ref", "--no-deref", name, commit],
    ))
    .await
    .map(drop)
}

/// Takes the checkpoint of `turn`: a snapshot under the ref of the turn.
///
/// # Errors
/// A [`GitError`] when a step of `git` fails.
pub async fn capture(git: &Git, worktree: &Worktree, turn: u32) -> Result<Checkpoint, GitError> {
    let commit = snapshot(git, worktree).await?;
    keep(git, worktree, &turn_ref(worktree.thread(), turn), &commit).await?;
    Ok(Checkpoint {
        turn,
        commit,
        at: chrono::Utc::now().timestamp(),
    })
}

/// The checkpoints of the thread of the worktree, the oldest first.
///
/// # Errors
/// A [`GitError`] when `git for-each-ref` fails.
pub async fn list(git: &Git, worktree: &Worktree) -> Result<Vec<Checkpoint>, GitError> {
    let under = format!("{}/turn/", refs_of(worktree.thread()));
    let text = git
        .text(Run::new(
            worktree.folder(),
            [
                "for-each-ref",
                "--format=%(refname) %(objectname) %(committerdate:unix)",
                under.as_str(),
            ],
        ))
        .await?;
    let mut found: Vec<Checkpoint> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(' ');
            let turn = parts.next()?.strip_prefix(under.as_str())?.parse().ok()?;
            Some(Checkpoint {
                turn,
                commit: parts.next()?.to_owned(),
                at: parts.next()?.parse().ok()?,
            })
        })
        .collect();
    found.sort_by_key(|checkpoint| checkpoint.turn);
    Ok(found)
}

/// Removes the checkpoints of the turns before `before` and what only they needed on the disk,
/// and gives the checkpoints that stay.
///
/// # Errors
/// A [`GitError`] when a step of `git` fails.
pub async fn forget_before(
    git: &Git,
    worktree: &Worktree,
    before: u32,
) -> Result<Vec<Checkpoint>, GitError> {
    forget(git, worktree, |turn| turn < before).await?;
    // The last reviewed snapshot goes: the next look at the changes makes a new one. The
    // undo of the last revert goes only with the last checkpoint.
    let mut old = vec![reviewed_ref(worktree.thread())];
    if list(git, worktree).await?.is_empty() {
        old.push(before_revert_ref(worktree.thread()));
    }
    for old in old {
        let gone = ["update-ref", "--no-deref", "-d", old.as_str()];
        git.output(Run::new(worktree.folder(), gone)).await?;
    }
    own::emptied(git, worktree).await?;
    list(git, worktree).await
}

/// Deletes the refs of the checkpoints for which `gone` says yes.
///
/// # Errors
/// A [`GitError`] when a step of `git` fails.
pub async fn forget(
    git: &Git,
    worktree: &Worktree,
    gone: impl Fn(u32) -> bool,
) -> Result<(), GitError> {
    for checkpoint in list(git, worktree).await? {
        if gone(checkpoint.turn) {
            let name = turn_ref(worktree.thread(), checkpoint.turn);
            git.output(Run::new(
                worktree.folder(),
                ["update-ref", "--no-deref", "-d", name.as_str()],
            ))
            .await?;
        }
    }
    Ok(())
}
