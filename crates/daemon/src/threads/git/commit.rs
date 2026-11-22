//! A commit that the person asked for (TH45): all files or the chosen ones, as they are in the
//! snapshot that the person looked at, on the branch of the thread or on a new one. The hooks
//! that the person accepted run ([`super::trust`]).
//!
//! I/O: `git read-tree` or `git restore --staged`, then `git commit`, held to the worktree.

use std::time::Duration;

use agent_threads::git::GitRefusal;

use super::place::{self, Worktree};
use super::run::{Git, Run};

/// The most bytes of a commit message.
const MOST: usize = 64 * 1024;

/// The time a commit gets: the hooks of the person can be slow.
const TIME: Duration = Duration::from_secs(600);

/// What the person asked to commit.
#[derive(Clone, Copy, Debug)]
pub struct Asked<'a> {
    pub message: &'a str,
    /// The snapshot of the files that the person looked at: what is committed is from it.
    pub reviewed: &'a str,
    /// `None` is each changed file.
    pub files: Option<&'a [String]>,
    pub new_branch: Option<&'a str>,
}

/// Commits and gives the ID of the commit with the branch it is on. The commit holds the files
/// as they are in the snapshot `asked.reviewed`, never an edit that came after it.
///
/// # Errors
/// A [`GitRefusal`] for an empty message, a path that leaves the worktree, a branch name that
/// is taken or not allowed, a branch that the runner did not make, a snapshot that is not of
/// the branch as it is now, nothing to commit, or a failure of `git`. `own` records a branch
/// that this commit made.
pub async fn make(
    git: &Git,
    worktree: &Worktree,
    asked: Asked<'_>,
    own: &dyn Fn(&str) -> Result<(), GitRefusal>,
) -> Result<(String, Option<String>), GitRefusal> {
    let dir = worktree.folder();
    let message = asked.message.trim();
    if message.is_empty() || message.len() > MOST {
        return Err(GitRefusal::EmptyMessage);
    }
    // Each check comes before the first change.
    let files = asked
        .files
        .map(|files| files.iter().map(|path| place::inside(path)).collect())
        .transpose()?;
    let new_branch = asked.new_branch.map(place::branch_name).transpose()?;
    let on = ["symbolic-ref", "--quiet", "--short", "HEAD"];
    if new_branch.is_none() {
        // A commit moves only a branch that the runner made for this thread (P12).
        let head = git.text(Run::new(dir, on)).await.ok();
        if !head.is_some_and(|head| worktree.owns(&head)) {
            return Err(GitRefusal::NotOurBranch);
        }
    }
    if let Some(name) = new_branch {
        let full = format!("refs/heads/{name}");
        let verify = ["show-ref", "--verify", "--quiet", full.as_str()];
        if git.yes(Run::new(dir, verify)).await? {
            return Err(GitRefusal::BranchExists);
        }
    }
    let reviewed = reviewed(git, worktree, asked.reviewed).await?;
    stage(git, worktree, reviewed, files).await?;
    let staged = ["diff", "--cached", "--quiet", "--no-ext-diff"];
    if git.yes(Run::new(dir, staged)).await? {
        return Err(GitRefusal::NothingToCommit);
    }
    if let Some(name) = new_branch {
        // `git switch --create` fails for a name that exists, so the branch is new and ours.
        git.output(Run::new(dir, ["switch", "--quiet", "--create", name]))
            .await?;
        own(name)?;
    }
    let message = format!("--message={message}");
    let commit = Run::new(dir, ["commit", "--quiet", message.as_str()]);
    git.output(commit.for_the_person().with_time(TIME)).await?;
    let commit = git.text(Run::new(dir, ["rev-parse", "HEAD"])).await?;
    Ok((commit, git.text(Run::new(dir, on)).await.ok()))
}

/// Checks that `id` is a snapshot of the branch as it is now: the ID of a commit whose parent
/// is the last commit of the branch (or that has none, in a repository with no commit).
async fn reviewed<'a>(git: &Git, worktree: &Worktree, id: &'a str) -> Result<&'a str, GitRefusal> {
    let dir = worktree.folder();
    let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
    if !matches!(id.len(), 40 | 64) || !id.chars().all(hex) {
        return Err(GitRefusal::NotReviewed);
    }
    let commit_of = |name: String| async move {
        let verify = ["rev-parse", "--verify", "--quiet", name.as_str()];
        git.text(Run::new(dir, verify)).await.ok()
    };
    if commit_of(format!("{id}^{{commit}}")).await.as_deref() != Some(id) {
        return Err(GitRefusal::NotReviewed);
    }
    let parent = commit_of(format!("{id}^1^{{commit}}")).await;
    if parent == commit_of("HEAD^{commit}".to_owned()).await {
        Ok(id)
    } else {
        Err(GitRefusal::NotReviewed)
    }
}

/// Makes the index hold the files of the snapshot `reviewed`: each one, or only the chosen
/// ones, which passed [`place::inside`], over the last commit. No file of the worktree is
/// read, so an edit after the snapshot is not in the commit and no filter program runs.
async fn stage(
    git: &Git,
    worktree: &Worktree,
    reviewed: &str,
    files: Option<Vec<&str>>,
) -> Result<(), GitRefusal> {
    let dir = worktree.folder();
    let Some(files) = files else {
        git.output(Run::new(dir, ["read-tree", reviewed])).await?;
        return Ok(());
    };
    if files.is_empty() {
        return Err(GitRefusal::NothingToCommit);
    }
    // The index holds only the chosen files: what was staged before is unstaged first. A
    // repository with no commit yet has nothing to unstage.
    let _ = git.output(Run::new(dir, ["reset", "--quiet"])).await;
    let mut paths = Vec::new();
    for path in files {
        paths.extend_from_slice(path.as_bytes());
        paths.push(0);
    }
    let source = format!("--source={reviewed}");
    let restore = [
        "--literal-pathspecs",
        "restore",
        source.as_str(),
        "--staged",
        "--pathspec-from-file=-",
        "--pathspec-file-nul",
    ];
    git.output(Run::new(dir, restore).with_input(paths)).await?;
    Ok(())
}
