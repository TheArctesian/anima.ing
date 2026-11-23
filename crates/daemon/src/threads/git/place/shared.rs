//! The checked copy of the history of one repository of the person (ruling of the operator):
//! one bare Git folder for each project in the folder of the records, shared by the threads
//! of that project. The Git folder of the runner for a worktree ([`super::own`]) reads it;
//! no `git` of the runner reads the object store of the person.
//!
//! An agent can write that object store, and can put a link to a different repository in the
//! place of any part of it. A `git` of the runner that reads it, with the rights of the
//! person, would then read a repository that the agent cannot read. So the store is read
//! with the rights of the thread: the runner gives a way to start a program as the agent of
//! a thread is started ([`AsThread`]), and that program makes one pack file of what is
//! missing, in a folder of its worktree. The runner takes the pack as a file, `git index-pack` names each object by the
//! hash of its content, and a commit counts as known only when each object of it is there.
//! Only the runner writes this folder, and nothing comes into it without that check.
//!
//! I/O: `git init`, `git index-pack`, `git rev-list`, one run of the thread, a few files.

pub mod pack;

use std::path::Path;
use std::time::Duration;

use super::super::project::refs;
use super::super::run::{Git, GitError, Run};
use super::Worktree;
use super::walk::{self, Dir};

/// The refs that say which thread proved which commit: `refs/anima/seen/<commit>/<thread>`.
/// The whole history of the commit is in the copy, and the thread showed that it can read
/// each object of it in the repository of the person: by bringing it, or by a look that reads
/// no content. A commit that a different thread brought is not known to a thread before that:
/// the other thread can have more rights.
const SEEN: &str = "refs/anima/seen";

/// The most commits that are named as known in one run of a thread.
const MOST_KNOWN: usize = 64;

/// The time the copy of a history gets: the first one of a large repository is long.
pub const LONG: Duration = Duration::from_hours(1);

/// The most bytes of the list of the commits where a shallow clone ends.
const MOST_SHALLOW: u64 = 16 * 1024 * 1024;

/// Makes the copy for the repository at `project` at `shared` when it is not there. `long_ids` is for a
/// repository whose IDs have 64 digits.
///
/// # Errors
/// A [`GitError`] when `git init` fails.
pub async fn ensure(
    git: &Git,
    (shared, project): (&Path, &Path),
    long_ids: bool,
) -> Result<(), GitError> {
    if shared.join("HEAD").is_file() {
        return Ok(());
    }
    super::own::init(git, shared, long_ids).await?;
    super::disk::name(shared, project).map_err(GitError::Files)
}

/// The refusal of a commit with an object that did not arrive under its ID.
fn forged() -> GitError {
    GitError::Failed {
        code: None,
        said: "an object of the repository is missing or is not what its ID says".to_owned(),
    }
}

fn failed(said: String) -> GitError {
    GitError::Failed { code: None, said }
}

/// The commits that the copy has whole: the ones that this thread proved, and the ones that
/// only a different thread proved.
async fn marked(store: &Git, worktree: &Worktree) -> Result<(Vec<String>, Vec<String>), GitError> {
    let listed = ["for-each-ref", "--format=%(refname)", SEEN];
    let listed = store
        .text(Run::new(&worktree.record().shared, listed))
        .await?;
    let thread = worktree.thread().as_simple().to_string();
    let (mut mine, mut others) = (Vec::new(), Vec::new());
    for name in listed.lines() {
        let mut parts = name.rsplit('/');
        let (Some(by), Some(commit)) = (parts.next(), parts.next().and_then(refs::id)) else {
            continue;
        };
        if by == thread {
            mine.push(commit.to_owned());
        } else {
            others.push(commit.to_owned());
        }
    }
    others.retain(|commit| !mine.contains(commit));
    others.dedup();
    Ok((mine, others))
}

/// Writes down that the thread of `worktree` proved `commit`.
async fn mark(store: &Git, worktree: &Worktree, commit: &str) -> Result<(), GitError> {
    let seen = format!("{SEEN}/{commit}/{}", worktree.thread().as_simple());
    let keep = ["update-ref", "--no-deref", seen.as_str(), commit];
    let keep = Run::new(&worktree.record().shared, keep);
    store.output(keep).await.map(drop)
}

/// Whether the copy alone has `commit` as a commit with each object of its history, but for
/// the history of `known`.
async fn whole(store: &Git, worktree: &Worktree, commit: &str, known: &[&str]) -> bool {
    let shared = &worktree.record().shared;
    let itself = format!("{commit}^{{commit}}");
    let is = [
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &itself,
    ];
    let is = store.text(Run::new(shared, is)).await;
    let mut all = vec!["rev-list", "--objects", "--quiet", commit, "--not"];
    all.extend(known);
    let all = Run::new(shared, all).with_time(LONG);
    is.ok().as_deref() == Some(commit) && store.output(all).await.is_ok()
}

/// Makes each commit of `commits` known to the thread of `worktree`: its whole history is in
/// the copy of the project, checked by the runner, and the thread showed that it can read it
/// in the repository of the person. What the copy lacks is read with the rights of the thread
/// and comes as a pack. `git` is the one of [`Git::for_worktree`]. A commit that the thread
/// proved before costs one look.
///
/// # Errors
/// A [`GitError`] when the runner has no way to run as the thread, the thread cannot read
/// the commit, a commit is no commit, or an object of it is missing or forged.
pub async fn learn(git: &Git, worktree: &Worktree, commits: &[&str]) -> Result<(), GitError> {
    let store = &git.in_own(&worktree.record().shared).await?;
    let (mut mine, others) = marked(store, worktree).await?;
    let mut wanted = Vec::new();
    for commit in commits {
        let commit = refs::id(commit).ok_or_else(forged)?;
        if !mine.iter().any(|known| known == commit) {
            wanted.push(commit);
        }
    }
    if wanted.is_empty() {
        return Ok(());
    }
    shallow(worktree)?;
    mine.truncate(MOST_KNOWN);
    // What a different thread brought: one look with the rights of this thread, which reads
    // no content, shows whether this thread can read it too.
    let mut there: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|commit| others.iter().any(|other| other == commit))
        .collect();
    if mine.is_empty() {
        there.extend(others.iter().map(String::as_str).take(MOST_KNOWN));
        there.dedup();
    }
    let known: Vec<&str> = mine.iter().map(String::as_str).collect();
    if !there.is_empty() && pack::reads(git, worktree, (&there, &known)).await? {
        for commit in &there {
            mark(store, worktree, commit).await?;
        }
        wanted.retain(|commit| !there.contains(commit));
        mine.extend(there.iter().map(|commit| (*commit).to_owned()));
    }
    let known: Vec<&str> = mine.iter().map(String::as_str).take(MOST_KNOWN).collect();
    for commit in wanted {
        brought(git, (store, worktree), commit, &known).await?;
        mark(store, worktree, commit).await?;
    }
    Ok(())
}

/// Has the thread bring the history of `commit` without the history of `known`, and checks
/// in the copy that each object of it is there. When that fails, the whole history again:
/// the repository of the person or the copy can have lost what was known.
async fn brought(
    git: &Git,
    (store, worktree): (&Git, &Worktree),
    commit: &str,
    known: &[&str],
) -> Result<(), GitError> {
    let first: Result<(), GitError> = async {
        pack::make(git, worktree, (&[commit], known)).await?;
        pack::take(store, worktree).await
    }
    .await;
    if first.is_ok() && whole(store, worktree, commit, known).await {
        return Ok(());
    }
    if known.is_empty() {
        return first.and(Err(forged()));
    }
    pack::make(git, worktree, (&[commit], &[])).await?;
    pack::take(store, worktree).await?;
    if whole(store, worktree, commit, &[]).await {
        Ok(())
    } else {
        Err(forged())
    }
}

/// Copies the list of the commits where a shallow clone ends into the Git folder of the
/// runner, so that `git` there does not look for their parents. Only lines that are IDs.
fn shallow(worktree: &Worktree) -> Result<(), GitError> {
    let record = worktree.record();
    let common = Dir::open(&record.common_dir).map_err(GitError::Files)?;
    let listed = common.read(walk::name("shallow"), MOST_SHALLOW);
    let listed = listed.map_err(GitError::Files)?.unwrap_or_default();
    let listed = String::from_utf8_lossy(&listed);
    let ends: String = listed
        .lines()
        .filter_map(refs::id)
        .flat_map(|id| [id, "\n"])
        .collect();
    // The copy of the project and the Git folder of the runner both stop at these commits.
    for folder in [&record.shared, &record.own] {
        let file = folder.join("shallow");
        if ends.is_empty() {
            let _ = std::fs::remove_file(file);
        } else {
            std::fs::write(file, &ends).map_err(GitError::Files)?;
        }
    }
    Ok(())
}
