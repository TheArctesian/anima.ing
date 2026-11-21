//! A commit that the person asked for (TH45): all files of the worktree or the chosen ones, on
//! the branch of the thread or on a new one. The hooks of the person run, as in a terminal.
//!
//! I/O: `git add`, `git commit`, held to the worktree.

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
    /// `None` is each changed file.
    pub files: Option<&'a [String]>,
    pub new_branch: Option<&'a str>,
}

/// Commits and gives the ID of the commit with the branch it is on.
///
/// # Errors
/// A [`GitRefusal`] for an empty message, a path that leaves the worktree, a branch name that
/// is taken or not allowed, a branch that the runner did not make, nothing to commit, or a
/// failure of `git`. `own` records a branch that this commit made.
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
    stage(git, worktree, files).await?;
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

/// Puts into the index each changed file, or only the chosen ones, which passed
/// [`place::inside`].
async fn stage(git: &Git, worktree: &Worktree, files: Option<Vec<&str>>) -> Result<(), GitRefusal> {
    let dir = worktree.folder();
    let Some(files) = files else {
        git.output(Run::new(dir, ["add", "--all", "--", "."]).for_the_person())
            .await?;
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
    let add = [
        "--literal-pathspecs",
        "add",
        "--all",
        "--pathspec-from-file=-",
        "--pathspec-file-nul",
    ];
    git.output(Run::new(dir, add).for_the_person().with_input(paths))
        .await?;
    Ok(())
}
