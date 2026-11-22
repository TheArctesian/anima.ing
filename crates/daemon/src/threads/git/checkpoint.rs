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
    // The private index starts from the last commit, never from the index of the worktree: an
    // agent can mark a file there as unchanged, and its edit would then be in no checkpoint.
    if let Some(head) = &head {
        git.output(with_index(Run::new(dir, ["read-tree", head.as_str()])))
            .await?;
    }
    // `git add` never looks into a nested repository that the last commit names: it would run
    // `git` there, with the configuration of that repository (THR-050). Such an entry stays
    // as the last commit has it.
    let mut add: Vec<String> = ["add", "--all", "--", "."].map(str::to_owned).into();
    for nested in nested(git, with_index(Run::new(dir, LIST))).await? {
        add.push(format!(":(exclude,literal){nested}"));
    }
    git.output(with_index(Run::new(dir, add))).await?;
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
