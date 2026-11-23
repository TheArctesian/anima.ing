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

/// The bytes that the copy of a project can always have. Above them it can have three times
/// the bytes of the object store of the project ([`room`]).
pub const ROOM: u64 = 1024 * 1024 * 1024;

/// How many times the size of the object store of the project its copy can have.
const TIMES: u64 = 3;

/// The bytes that stay free on the disk of the records, whatever a project brings.
const MIN_FREE: u64 = 1024 * 1024 * 1024;

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

/// Whether the Git folder at `folder` (the `git` of `store` runs with it) has `commit` as a
/// commit with each object of its history, but for the history of `known`.
async fn whole(store: &Git, folder: &Path, commit: &str, known: &[&str]) -> bool {
    let itself = format!("{commit}^{{commit}}");
    let is = [
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &itself,
    ];
    let is = store.text(Run::new(folder, is)).await;
    let mut all = vec!["rev-list", "--objects", "--quiet", commit, "--not"];
    all.extend(known);
    let all = Run::new(folder, all).with_time(LONG);
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

/// The bytes that the copy of the project of `worktree` can still take. Each thread of each
/// project writes to the one disk of the records, and what a thread brings is bytes that its
/// agent chose. So the copy of a project is held to a size that follows the project: three
/// times the disk that the object store of the person really uses, and the room that each
/// project has ([`ROOM`]). An agent that wants the runner to keep more must use that much of
/// the disk of the project itself, with real bytes. The disk of the records also keeps
/// [`MIN_FREE`] free.
///
/// # Errors
/// A [`GitError`] when nothing is left: nothing more is brought.
async fn left(git: &Git, worktree: &Worktree) -> Result<u64, GitError> {
    let record = worktree.record();
    let common = record.common_dir.clone();
    // The store of the person can be large: it is counted off the thread of the calls.
    let theirs = tokio::task::spawn_blocking(move || {
        let objects = Dir::open(&common)
            .and_then(|common| common.below(Path::new("objects"), walk::Make::No));
        objects.ok().flatten().map_or(0, |objects| objects.used())
    });
    let theirs = theirs.await.unwrap_or(0);
    let most = theirs.saturating_mul(TIMES).saturating_add(git.copy_room());
    let used = super::disk::bytes(&record.shared);
    let free = free_at(&record.shared).saturating_sub(MIN_FREE);
    match most.saturating_sub(used).min(free) {
        0 => Err(failed(
            "the copy of the history of this project reached the size that the project \
             allows, or the disk of Anima's records is nearly full. Remove the worktrees \
             of this project, then remove its copy; the next chat builds it again"
                .to_owned(),
        )),
        left => Ok(left),
    }
}

/// The bytes that are free on the disk that holds `folder`.
#[cfg(unix)]
fn free_at(folder: &Path) -> u64 {
    rustix::fs::statvfs(folder).map_or(0, |disk| disk.f_bavail.saturating_mul(disk.f_frsize))
}

#[cfg(not(unix))]
const fn free_at(_folder: &Path) -> u64 {
    u64::MAX
}

/// A Git folder of the runner beside the copy, for one pack of a thread: what the thread left
/// is named and checked in it, and only what a known commit needs goes on into the copy. It
/// is removed when the value is dropped, on each way out.
struct Quarantine(std::path::PathBuf);

impl Drop for Quarantine {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Has the thread bring the history of `commit` without the history of `known`. When that
/// gives no whole history, the whole history again: the repository of the person or the copy
/// can have lost what was known.
async fn brought(
    git: &Git,
    (store, worktree): (&Git, &Worktree),
    commit: &str,
    known: &[&str],
) -> Result<(), GitError> {
    let first = once(git, (store, worktree), commit, known).await;
    if first.is_ok() || known.is_empty() {
        return first;
    }
    once(git, (store, worktree), commit, &[]).await
}

/// One try of [`brought`]. The pack of the thread goes into a quarantine, never into the
/// copy: `git index-pack` names its objects there, the history of `commit` is checked there,
/// and the runner's own `git pack-objects` then writes into the copy only the objects of
/// that history that the copy lacks. What a thread left besides them never comes into the
/// copy, and nothing is ever removed from the copy.
async fn once(
    git: &Git,
    (store, worktree): (&Git, &Worktree),
    commit: &str,
    known: &[&str],
) -> Result<(), GitError> {
    let record = worktree.record();
    let left = left(git, worktree).await?;
    pack::make(git, worktree, (&[commit], known)).await?;
    let name = format!("quarantine-{}.git", worktree.thread().as_simple());
    let held = Quarantine(record.shared.join(name));
    let folder = held.0.as_path();
    let free = git.unbound();
    super::own::make(&free, folder, &record.shared, commit.len() == 64).await?;
    // The copy can end at the commits of a shallow clone: the quarantine ends there too.
    if let Ok(ends) = std::fs::read(record.shared.join("shallow")) {
        std::fs::write(folder.join("shallow"), ends).map_err(GitError::Files)?;
    }
    let checked = &git.in_own(folder).await?;
    pack::take(checked, (worktree, folder), left).await?;
    if !whole(checked, folder, commit, known).await {
        return Err(forged());
    }
    let mut wanted = format!("{commit}\n");
    for commit in known {
        wanted.extend(["^", commit, "\n"]);
    }
    let out = super::own::fresh(folder, "out")?;
    let base = out.join("pack");
    // `--local`: only what is in the quarantine, not what the copy has already.
    let pack: [&std::ffi::OsStr; 5] = [
        "pack-objects".as_ref(),
        "--revs".as_ref(),
        "--local".as_ref(),
        "--quiet".as_ref(),
        base.as_os_str(),
    ];
    let pack = Run::new(folder, pack).with_time(LONG);
    checked.output(pack.with_input(wanted.into_bytes())).await?;
    if super::disk::bytes(&out) > left {
        return Err(failed(
            "the history of this commit is larger than the copy of this project can hold"
                .to_owned(),
        ));
    }
    let made = std::fs::read_dir(&out).map_err(GitError::Files)?;
    for file in made.flatten() {
        let placed = record.shared.join("objects/pack").join(file.file_name());
        // A pack of this name that is there holds the same bytes: it stays.
        if !placed.exists() {
            std::fs::rename(file.path(), placed).map_err(GitError::Files)?;
        }
    }
    // The copy alone has the whole history now, or the commit is not known.
    if whole(store, &record.shared, commit, known).await {
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
