//! A commit that the person asked for (TH45): all files or the chosen ones, as they are in the
//! snapshot that the person looked at, on the branch of the thread or on a new one. //!
//! The commit is made from the tree of the snapshot with `git commit-tree` and the branch is
//! moved with `git update-ref`: a merge or a cherry-pick that somebody left half done in the
//! Git folder cannot give the commit a second parent or a different author, and no file of
//! the worktree is read. `git commit` is not used, so no `pre-commit` hook runs.
//!
//! I/O: `git commit-tree`, `git update-ref`, a private index for chosen files.

use std::time::Duration;

use agent_threads::git::GitRefusal;

use super::checkpoint;
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
/// as they are in the snapshot `asked.reviewed`, never an edit that came after it, and has one
/// parent: the branch as it was.
///
/// # Errors
/// A [`GitRefusal`] for an empty message, a path that leaves the worktree, a branch name that
/// is taken or not allowed, a branch that the runner did not make, a snapshot that is not the
/// one the person last looked at, nothing to commit, or a failure of `git`. `own` records a
/// branch that this commit made.
pub async fn make(
    git: &Git,
    worktree: &Worktree,
    asked: Asked<'_>,
    own: &(dyn Fn(&str) -> Result<(), GitRefusal> + Sync),
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
    let branch = branch_for(git, worktree, asked.new_branch).await?;
    let head = commit_of(git, worktree, "HEAD^{commit}").await;
    let reviewed = reviewed(git, worktree, asked.reviewed, head.as_deref()).await?;
    let tree = tree_of(git, worktree, (reviewed, head.as_deref()), files).await?;
    if let Some(head) = &head
        && commit_of(git, worktree, &format!("{head}^{{tree}}"))
            .await
            .as_deref()
            == Some(&tree)
    {
        return Err(GitRefusal::NothingToCommit);
    }
    let mut write = vec!["commit-tree", tree.as_str(), "-m", message];
    if let Some(head) = &head {
        write.extend(["-p", head.as_str()]);
    }
    if git.signs() {
        write.push("-S");
    }
    let write = Run::new(dir, write).for_the_person().with_time(TIME);
    let commit = git.text(write).await?;
    // The branch moves only from where it was: a new branch must not exist, the branch of the
    // thread must still be at the parent of the commit.
    let none = "0".repeat(commit.len());
    let before = match (asked.new_branch, &head) {
        (None, Some(head)) => head.as_str(),
        _ => none.as_str(),
    };
    let name = format!("refs/heads/{branch}");
    let land = ["update-ref", name.as_str(), commit.as_str(), before];
    git.output(Run::new(dir, land).for_the_person().with_time(TIME))
        .await?;
    if asked.new_branch.is_some() {
        own(&branch)?;
        git.output(Run::new(dir, ["symbolic-ref", "HEAD", name.as_str()]))
            .await?;
    }
    // The index follows the branch; the files of the worktree stay as they are.
    git.output(Run::new(dir, ["reset", "--quiet"])).await?;
    Ok((commit, Some(branch)))
}

/// The branch the commit goes to: a new one that does not exist yet, or the one the worktree
/// is on when the runner made it for this thread (P12).
async fn branch_for(
    git: &Git,
    worktree: &Worktree,
    new_branch: Option<&str>,
) -> Result<String, GitRefusal> {
    let dir = worktree.folder();
    let Some(name) = new_branch else {
        let on = ["symbolic-ref", "--quiet", "--short", "HEAD"];
        let head = git.text(Run::new(dir, on)).await.ok();
        return head
            .filter(|head| worktree.owns(head))
            .ok_or(GitRefusal::NotOurBranch);
    };
    let name = place::branch_name(name)?;
    let full = format!("refs/heads/{name}");
    let verify = ["show-ref", "--verify", "--quiet", full.as_str()];
    if git.yes(Run::new(dir, verify)).await? {
        return Err(GitRefusal::BranchExists);
    }
    Ok(name.to_owned())
}

/// The ID that `name` resolves to; `None` when it names nothing.
async fn commit_of(git: &Git, worktree: &Worktree, name: &str) -> Option<String> {
    let verify = ["rev-parse", "--verify", "--quiet", "--end-of-options", name];
    git.text(Run::new(worktree.folder(), verify)).await.ok()
}

/// Checks that `id` is the snapshot that the person last looked at, of the branch as it is
/// now: the ID under the ref that the runner wrote for it, with the last commit of the branch
/// as its parent (or none, in a repository with no commit).
async fn reviewed<'a>(
    git: &Git,
    worktree: &Worktree,
    id: &'a str,
    head: Option<&str>,
) -> Result<&'a str, GitRefusal> {
    let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
    if !matches!(id.len(), 40 | 64) || !id.chars().all(hex) {
        return Err(GitRefusal::NotReviewed);
    }
    let kept = checkpoint::reviewed_ref(worktree.thread());
    let shown = commit_of(git, worktree, &format!("{kept}^{{commit}}")).await;
    let parent = commit_of(git, worktree, &format!("{id}^1^{{commit}}")).await;
    if shown.as_deref() == Some(id) && parent.as_deref() == head {
        Ok(id)
    } else {
        Err(GitRefusal::NotReviewed)
    }
}

/// The tree to commit: the one of the snapshot, or the last commit with only the chosen files
/// as the snapshot has them, which passed [`place::inside`]. It is built in a private index:
/// no file and no index of the worktree is read or changed, and no filter program runs.
async fn tree_of<'a>(
    git: &Git,
    worktree: &'a Worktree,
    (reviewed, head): (&str, Option<&str>),
    files: Option<Vec<&str>>,
) -> Result<String, GitRefusal> {
    let dir = worktree.folder();
    let Some(files) = files else {
        let whole = format!("{reviewed}^{{tree}}");
        return commit_of(git, worktree, &whole)
            .await
            .ok_or(GitRefusal::NotReviewed);
    };
    if files.is_empty() {
        return Err(GitRefusal::NothingToCommit);
    }
    let scratch = tempfile::tempdir().map_err(|_| GitRefusal::BadPath)?;
    let index = scratch.path().join("index");
    let private = |run: Run<'a>| run.with_env("GIT_INDEX_FILE", index.as_os_str());
    if let Some(head) = head {
        git.output(private(Run::new(dir, ["read-tree", head])))
            .await?;
    }
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
    git.output(private(Run::new(dir, restore).with_input(paths)))
        .await?;
    Ok(git.text(private(Run::new(dir, ["write-tree"]))).await?)
}
