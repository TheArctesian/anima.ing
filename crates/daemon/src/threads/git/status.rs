//! How the branch of a worktree stands (TH45): its name, its upstream, ahead and behind, the
//! default branch, and the files that changed since the last commit.
//!
//! The branch and the refs are read as files ([`refs`]); the changed files are the difference
//! between the last commit and the files of now, taken with the `git` of the runner.

use agent_threads::git::Status;

use super::changes::{self, Look};
use super::checkpoint;
use super::place::{self, Worktree, own};
use super::project::refs;
use super::run::settings::{self, Setting};
use super::run::{Git, GitError, Run, said};

/// The status of the worktree. `listed` is the configuration of the repository of the person
/// as text ([`super::project::settings`]).
///
/// # Errors
/// A [`GitError`] when the branch cannot be read or a command of `git` fails.
pub async fn of(git: &Git, worktree: &Worktree, listed: &[Setting]) -> Result<Status, GitError> {
    let layout = worktree.layout();
    let common = layout.common_dir.as_path();
    let head = own::tip(git, worktree).await?;
    let mut status = files(git, worktree, head.commit.as_deref()).await?;
    let remote = remote(listed);
    status.has_remote = remote.is_some();
    status.push_url = remote
        .as_ref()
        .map(|(_, url)| said::without_credentials(url));
    let remote = remote.map(|(name, _)| name);
    status.default_branch = default_branch(common, remote.as_deref())?;
    // What the remote says is the default can be changed in the repository, so the two usual
    // names count as the default branch too.
    status.is_default_branch = head.branch.as_deref().is_some_and(|branch| {
        Some(branch) == status.default_branch.as_deref() || matches!(branch, "main" | "master")
    });
    let count = |from: Option<String>, to: Option<String>| async move {
        match (from, to) {
            (Some(from), Some(to)) => ahead(git, worktree, &from, &to).await,
            _ => 0,
        }
    };
    if let (Some(remote), Some(branch)) = (&remote, &head.branch) {
        let tracked = refs::resolve(common, &format!("refs/remotes/{remote}/{branch}"))?;
        let sent = sent(git, worktree, branch).await;
        if sent.is_some() || tracked.is_some() {
            status.upstream = Some(format!("{remote}/{branch}"));
        }
        status.ahead = count(sent.or_else(|| tracked.clone()), head.commit.clone()).await;
        status.behind = count(head.commit.clone(), tracked).await;
    }
    if let Some(default) = &status.default_branch {
        let tracked = match &remote {
            Some(remote) => refs::resolve(common, &format!("refs/remotes/{remote}/{default}"))?,
            None => None,
        };
        let base = match tracked {
            Some(tracked) => Some(tracked),
            None => refs::resolve(common, &format!("refs/heads/{default}"))?,
        };
        status.ahead_of_default = count(base, head.commit.clone()).await;
    }
    status.branch = head.branch;
    Ok(status)
}

/// The files that differ from the last commit `tip`, with their counts.
async fn files(git: &Git, worktree: &Worktree, tip: Option<&str>) -> Result<Status, GitError> {
    let dir = worktree.folder();
    let now = checkpoint::tree_now(git, worktree, tip).await?;
    let base = if let Some(tip) = tip {
        tip.to_owned()
    } else {
        // A repository with no commit yet: each file is new.
        let empty = ["hash-object", "-t", "tree", "--stdin"];
        let empty = Run::new(dir, empty).with_input(Vec::new());
        git.text(empty).await?
    };
    let changed = changes::between(git, worktree, (&base, &now), Look::Counts).await?;
    Ok(Status {
        files: changed.files,
        additions: changed.additions,
        deletions: changed.deletions,
        ..Status::default()
    })
}

/// The remote a push goes to with its URL, when its name is safe as a part of a ref.
#[must_use]
pub fn remote(listed: &[Setting]) -> Option<(String, String)> {
    settings::push_remote(listed)
        .filter(|(name, _)| place::branch_name(name).is_ok() && !name.contains('/'))
}

/// The commit of `branch` that the last push of the runner sent; `None` before the first.
pub async fn sent(git: &Git, worktree: &Worktree, branch: &str) -> Option<String> {
    let name = format!("{}^{{commit}}", checkpoint::pushed_ref(branch));
    let verify = [
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &name,
    ];
    git.text(Run::new(worktree.folder(), verify)).await.ok()
}

/// The default branch: what the remote says its `HEAD` is, else `main`, else `master`.
fn default_branch(
    common: &std::path::Path,
    remote: Option<&str>,
) -> Result<Option<String>, GitError> {
    if let Some(remote) = remote {
        let under = format!("refs/remotes/{remote}/");
        let named = refs::names(common, &format!("{under}HEAD"))?;
        if let Some(branch) = named.as_deref().and_then(|to| to.strip_prefix(&under)) {
            return Ok(Some(branch.to_owned()));
        }
    }
    for name in ["main", "master"] {
        if refs::resolve(common, &format!("refs/heads/{name}"))?.is_some() {
            return Ok(Some(name.to_owned()));
        }
    }
    Ok(None)
}

/// The count of the commits of `to` that `from` does not have; 0 when one is not a commit
/// that the runner can read.
async fn ahead(git: &Git, worktree: &Worktree, from: &str, to: &str) -> u32 {
    let range = format!("{from}..{to}");
    let count = ["rev-list", "--count", "--end-of-options", &range, "--"];
    // The history is only in the repository of the person; a count decides nothing alone.
    let count = own::borrowing(
        Run::new(worktree.folder(), count),
        &worktree.record().common_dir,
    );
    let count = git.text(count).await;
    count.ok().and_then(|count| count.parse().ok()).unwrap_or(0)
}
