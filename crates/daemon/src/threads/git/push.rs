//! A push that the person asked for (TH45): the branch of the worktree to its remote, never by
//! force, and the default branch only with a yes of its own.
//!
//! I/O: `git push` from the Git folder of the runner, with the credentials that git itself
//! finds; the runner reads none.

use agent_threads::git::{GitOutcome, GitRefusal, Status};

pub mod pull_request;

use super::place::{self, Worktree, own};
use super::run::hooks::Hooks;
use super::run::settings::Setting;
use super::run::{Git, REMOTE_TIME, Run};
use super::{checkpoint, status};

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

/// A push that passed each check that needs no network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    remote: String,
    /// The URL as the repository has it; it can hold a credential, so it is never shown.
    url: String,
    /// The URL for the page, with no credential.
    shown: String,
    branch: String,
    commit: String,
    up_to_date: bool,
}

/// Refuses a URL that `git` would not reach over HTTPS or SSH: `ext::` and each other
/// `<program>::<address>` start a program of that name, and a path is a folder of this
/// computer whose hooks would run. `local` allows a path, for the tests.
///
/// # Errors
/// [`GitRefusal::Failed`] with the reason.
pub fn destination(url: &str, local: bool) -> Result<(), GitRefusal> {
    let by_scheme = ["https://", "http://", "ssh://"]
        .iter()
        .any(|scheme| url.starts_with(scheme));
    // `user@host:path`, as `ssh` reads it: a host, then one `:`, with no `/` before it.
    let host = url.split_once(':').map_or("", |(host, _)| host);
    let host_like = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '@');
    let by_ssh = !host.is_empty()
        && !host.starts_with('-')
        && host.chars().all(host_like)
        && !url.contains("::")
        && !url.contains("://");
    let by_path = local && std::path::Path::new(url).is_absolute();
    if (by_scheme || by_ssh || by_path) && !url.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(GitRefusal::Failed {
            said: "the remote of the repository is not an HTTPS or SSH address;                    Anima pushes to no other kind"
                .to_owned(),
        })
    }
}

/// Checks a push of the branch of the worktree and reads what it sends where. `listed` is the
/// configuration of the repository of the person as text.
///
/// # Errors
/// A [`GitRefusal`] from [`allowed`], for a branch that the runner did not make, for a
/// remote that is not allowed, or when the branch cannot be read.
pub async fn ready(
    git: &Git,
    worktree: &Worktree,
    listed: &[Setting],
    default_branch_allowed: bool,
) -> Result<Ready, GitRefusal> {
    let now = status::of(git, worktree, listed).await?;
    let branch = allowed(&now, default_branch_allowed)?.to_owned();
    // A push moves only a branch that the runner made for this thread (P12).
    if !worktree.owns(&branch) {
        return Err(GitRefusal::NotOurBranch);
    }
    let (remote, url) = status::remote(listed).ok_or(GitRefusal::NoRemote)?;
    destination(&url, git.local_remotes())?;
    let commit = own::tip(git, worktree).await?.commit;
    Ok(Ready {
        remote,
        shown: now.push_url.unwrap_or_default(),
        url,
        branch,
        commit: commit.ok_or(GitRefusal::NoCommits)?,
        up_to_date: now.upstream.is_some() && now.ahead == 0,
    })
}

/// Pushes what [`ready`] found, from the Git folder of the runner to the URL itself: the
/// repository of the person is not asked, so no rewrite of a URL, no proxy, no helper and no
/// hook that is set there applies. The credentials are the ones that git finds for the person.
///
/// # Errors
/// The words of `git` when the push fails.
pub async fn send(
    git: &Git,
    worktree: &Worktree,
    ready: Ready,
    hooks: Option<Hooks<'_>>,
) -> Result<GitOutcome, GitRefusal> {
    let dir = worktree.folder();
    let pushed = |up_to_date| GitOutcome::Pushed {
        remote: ready.remote.clone(),
        url: ready.shown.clone(),
        branch: ready.branch.clone(),
        up_to_date,
    };
    if ready.up_to_date {
        return Ok(pushed(true));
    }
    if let Some(hooks) = hooks {
        let to = (ready.remote.as_str(), ready.shown.as_str());
        let what = (ready.branch.as_str(), ready.commit.as_str());
        hooks.before_push(to, what).await?;
    }
    let target = format!("{}:refs/heads/{}", ready.commit, ready.branch);
    // The program at the remote is the one of git; no tag, no submodule and no option goes
    // with the branch, and the remote is asked for nothing first.
    let push = [
        "-c",
        "push.recurseSubmodules=no",
        "-c",
        "push.followTags=false",
        "-c",
        "push.negotiate=false",
        "-c",
        "push.pushOption=",
        "-c",
        "protocol.git.allow=never",
        "push",
        "--receive-pack=git-receive-pack",
        "--",
        THERE,
        target.as_str(),
    ];
    // A push needs the history, which only the repository of the person has; the remote
    // checks each object that it gets.
    let push = own::borrowing(Run::new(dir, push), &worktree.record().common_dir)
        .for_the_person()
        .with_time(REMOTE_TIME)
        // In the environment: a URL can hold a credential, and each user reads arguments.
        .with_setting(&format!("remote.{THERE}.url"), &ready.url);
    git.output(push).await?;
    let sent = checkpoint::pushed_ref(&ready.branch);
    let keep = [
        "update-ref",
        "--no-deref",
        sent.as_str(),
        ready.commit.as_str(),
    ];
    git.output(Run::new(dir, keep)).await?;
    Ok(pushed(false))
}

/// The name of the remote of one push in the configuration of that run alone.
const THERE: &str = "anima-push";

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

    #[test]
    fn only_an_https_or_ssh_address_is_a_place_to_push_to() {
        for good in [
            "https://github.com/ada/engine.git",
            "ssh://git@github.com/ada/engine.git",
            "ssh://[::1]:22/ada/engine.git",
            "git@github.com:ada/engine.git",
            "host:path/engine.git",
        ] {
            assert_eq!(destination(good, false), Ok(()), "{good}");
        }
        for bad in [
            "ext::sh -c 'touch RAN'",
            "fd::3",
            "evil::https://github.com/ada/engine.git",
            "/home/ada/engine.git",
            "file:///home/ada/engine.git",
            "git://github.com/ada/engine.git",
            "../engine.git",
            "-oProxyCommand=evil:x",
            "",
        ] {
            assert!(destination(bad, false).is_err(), "{bad}");
        }
        assert_eq!(destination("/home/ada/engine.git", true), Ok(()));
    }
}
