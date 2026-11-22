//! A push that the person asked for (TH45): the branch of the worktree to its remote, never by
//! force, and the default branch only with a yes of its own.
//!
//! I/O: `git push` with the credentials that git itself finds; the runner reads none.

use agent_threads::git::{GitOutcome, GitRefusal, Status};

use super::place::{self, Worktree};
use super::run::{Git, REMOTE_TIME, Run};
use super::status;

/// The checks of a push that need no network, from the status of the branch.
///
/// # Errors
/// The [`GitRefusal`] that the page shows as the reason.
pub fn allowed(status: &Status, default_branch_allowed: bool) -> Result<&str, GitRefusal> {
    let branch = status.branch.as_deref().ok_or(GitRefusal::DetachedHead)?;
    place::branch_name(branch)?;
    if !status.has_remote {
        return Err(GitRefusal::NoRemote);
    }
    if status.behind > 0 {
        return Err(GitRefusal::Behind);
    }
    if status.is_default_branch && !default_branch_allowed {
        return Err(GitRefusal::DefaultBranch);
    }
    Ok(branch)
}

/// Pushes the branch of the worktree and records its upstream.
///
/// # Errors
/// A [`GitRefusal`] from [`allowed`], for a branch that the runner did not make, or the words
/// of `git` when the push fails.
pub async fn make(
    git: &Git,
    worktree: &Worktree,
    default_branch_allowed: bool,
) -> Result<GitOutcome, GitRefusal> {
    let dir = worktree.folder();
    let now = status::of(git, worktree).await?;
    let branch = allowed(&now, default_branch_allowed)?.to_owned();
    // A push moves only a branch that the runner made for this thread (P12).
    if !worktree.owns(&branch) {
        return Err(GitRefusal::NotOurBranch);
    }
    let remote = status::remote(git, worktree)
        .await?
        .ok_or(GitRefusal::NoRemote)?;
    let pushed = |up_to_date| GitOutcome::Pushed {
        remote: remote.clone(),
        url: now.push_url.clone().unwrap_or_default(),
        branch: branch.clone(),
        up_to_date,
    };
    if now.upstream.is_some() && now.ahead == 0 {
        return Ok(pushed(true));
    }
    let target = format!("HEAD:refs/heads/{branch}");
    // The programs at the remote are the ones of git, whatever the repository says.
    let receive = format!("remote.{remote}.receivepack=git-receive-pack");
    let upload = format!("remote.{remote}.uploadpack=git-upload-pack");
    // The program at the remote is the one of git, not one that the configuration names; no
    // tag and no submodule goes with the branch.
    let push = [
        "-c",
        "push.recurseSubmodules=no",
        "-c",
        "push.followTags=false",
        "-c",
        receive.as_str(),
        "-c",
        upload.as_str(),
        "push",
        "--receive-pack=git-receive-pack",
        "--",
        remote.as_str(),
        target.as_str(),
    ];
    git.output(Run::new(dir, push).for_the_person().with_time(REMOTE_TIME))
        .await?;
    let merge = format!("refs/heads/{branch}");
    for (name, value) in [("remote", remote.as_str()), ("merge", merge.as_str())] {
        let key = format!("branch.{branch}.{name}");
        git.output(Run::new(dir, ["config", "--local", key.as_str(), value]))
            .await?;
    }
    Ok(pushed(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(name: &str) -> Status {
        Status {
            branch: Some(name.to_owned()),
            default_branch: Some("main".to_owned()),
            is_default_branch: name == "main",
            has_remote: true,
            ..Status::default()
        }
    }

    #[test]
    fn a_push_is_refused_for_each_state_that_cannot_be_pushed() {
        assert_eq!(
            allowed(&branch("anima/1234abcd"), false),
            Ok("anima/1234abcd")
        );
        let detached = Status {
            branch: None,
            ..branch("x")
        };
        assert_eq!(allowed(&detached, true), Err(GitRefusal::DetachedHead));
        let alone = Status {
            has_remote: false,
            ..branch("x")
        };
        assert_eq!(allowed(&alone, true), Err(GitRefusal::NoRemote));
        let behind = Status {
            behind: 1,
            ..branch("x")
        };
        assert_eq!(allowed(&behind, true), Err(GitRefusal::Behind));
    }

    #[test]
    fn the_default_branch_is_pushed_only_with_a_yes_of_its_own() {
        assert_eq!(
            allowed(&branch("main"), false),
            Err(GitRefusal::DefaultBranch)
        );
        assert_eq!(allowed(&branch("main"), true), Ok("main"));
    }
}
