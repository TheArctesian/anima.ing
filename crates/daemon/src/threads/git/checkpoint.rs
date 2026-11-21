//! Checkpoints (TH42): the files of a worktree at the end of a turn, kept as a commit under a
//! private ref of Anima. The index and the branch of the worktree do not move, and a file that
//! git ignores is not taken (THR-050).
//!
//! I/O: runs `git` with a private index file in a temporary folder.

use agent_threads::git::Checkpoint;
use uuid::Uuid;

use super::place::Worktree;
use super::run::{Git, GitError, Run};

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

/// The ref that keeps the work of a worktree that was removed (THR-097).
#[must_use]
pub fn kept_ref(thread: Uuid) -> String {
    format!("refs/anima/kept/{}", thread.as_hyphenated())
}

/// Makes a commit of the files of the worktree as they are now, with no ref and no change of
/// the index or the branch, and gives its ID.
///
/// # Errors
/// A [`GitError`] when a step of `git` fails or the temporary folder cannot be made.
pub async fn snapshot(git: &Git, worktree: &Worktree) -> Result<String, GitError> {
    let dir = worktree.folder();
    let scratch = tempfile::tempdir().map_err(GitError::Start)?;
    let index = scratch.path().join("index");
    let with_index = |run| private_index(run, &index);
    let head = match git
        .text(Run::new(
            dir,
            ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
        ))
        .await
    {
        Ok(head) => Some(head),
        Err(GitError::Failed { code: Some(1), .. }) => None,
        Err(other) => return Err(other),
    };
    if let Some(head) = &head {
        git.output(with_index(Run::new(dir, ["read-tree", head.as_str()])))
            .await?;
    }
    git.output(with_index(Run::new(dir, ["add", "--all", "--", "."])))
        .await?;
    let tree = git.text(with_index(Run::new(dir, ["write-tree"]))).await?;
    let mut commit = vec!["commit-tree".to_owned(), tree];
    if let Some(head) = head {
        commit.extend(["-p".to_owned(), head]);
    }
    commit.extend(["-m".to_owned(), "anima checkpoint".to_owned()]);
    let mut run = Run::new(dir, commit);
    for (name, value) in WHO {
        run = run.with_env(name, value);
    }
    git.text(run).await
}

/// A run that reads and writes the private index at `index`, never the index of the worktree.
fn private_index<'a>(run: Run<'a>, index: &std::path::Path) -> Run<'a> {
    run.with_env("GIT_INDEX_FILE", index.as_os_str())
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
