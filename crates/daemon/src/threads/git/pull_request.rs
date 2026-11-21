//! The pull request of a thread (TH45): the checks of the branch, the base branch, and the page
//! of the forge where the person opens it. The runner calls no forge and reads no token: the
//! person opens the pull request in their own browser.
//!
//! [`page`] is pure; [`prepare`] reads the status and the URL of the remote.

use std::fmt::Write as _;

use agent_threads::git::{GitRefusal, PullRequestPage};

use super::place::Worktree;
use super::run::{Git, Run};
use super::status;

/// The page of a forge that compares `head` with `base`, from the URL of the remote. `None` for
/// a forge that is not known, or a URL whose parts have a character that is not allowed.
#[must_use]
pub fn page(remote_url: &str, base: &str, head: &str) -> Option<String> {
    let (host, path) = host_and_path(remote_url)?;
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let plain = |part: &str| {
        !part.is_empty()
            && !part.starts_with('.')
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !path.split('/').all(plain) || path.split('/').count() < 2 {
        return None;
    }
    let (base, head) = (in_url(base), in_url(head));
    match host {
        "github.com" if path.split('/').count() == 2 => Some(format!(
            "https://github.com/{path}/compare/{base}...{head}?expand=1"
        )),
        "gitlab.com" => Some(format!(
            "https://gitlab.com/{path}/-/merge_requests/new?merge_request%5Bsource_branch%5D={head}&merge_request%5Btarget_branch%5D={base}"
        )),
        _ => None,
    }
}

/// The host and the path of a remote: `https://host/path`, `ssh://git@host/path`, `git@host:path`.
fn host_and_path(url: &str) -> Option<(&str, &str)> {
    let rest = ["https://", "ssh://"]
        .iter()
        .find_map(|scheme| url.strip_prefix(scheme));
    let (host, path) = match rest {
        Some(rest) => rest.split_once('/')?,
        None if !url.contains("://") => url.split_once(':')?,
        None => return None,
    };
    // A name before the host (`git@`, or a credential) is dropped and never used.
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    Some((host, path))
}

/// A branch name as a part of a URL: each byte outside letters, digits and `-_./` is escaped.
fn in_url(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/') {
            out.push(char::from(byte));
        } else {
            // Writing to a `String` cannot fail.
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// Checks that the branch can have a pull request and gives the page for it.
///
/// # Errors
/// The [`GitRefusal`] that the page shows as the reason.
pub async fn prepare(git: &Git, worktree: &Worktree) -> Result<PullRequestPage, GitRefusal> {
    let now = status::of(git, worktree).await?;
    let head = now.branch.clone().ok_or(GitRefusal::DetachedHead)?;
    let base = now.default_branch.clone().ok_or(GitRefusal::NoRemote)?;
    if now.is_default_branch {
        return Err(GitRefusal::DefaultBranch);
    }
    let remote = status::remote(git, worktree)
        .await?
        .ok_or(GitRefusal::NoRemote)?;
    if now.upstream.is_none() || now.ahead > 0 {
        return Err(GitRefusal::NotPushed);
    }
    if now.ahead_of_default == 0 {
        return Err(GitRefusal::NoCommits);
    }
    let url = git
        .text(Run::new(
            worktree.folder(),
            ["remote", "get-url", "--", remote.as_str()],
        ))
        .await?;
    let url = page(&url, &base, &head).ok_or(GitRefusal::UnknownForge)?;
    Ok(PullRequestPage { url, base, head })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_form_of_a_github_remote_gives_the_same_compare_page() {
        let want = "https://github.com/ada/engine/compare/main...anima/1234abcd?expand=1";
        for remote in [
            "https://github.com/ada/engine.git",
            "https://github.com/ada/engine",
            "git@github.com:ada/engine.git",
            "ssh://git@github.com/ada/engine.git",
        ] {
            assert_eq!(
                page(remote, "main", "anima/1234abcd").as_deref(),
                Some(want),
                "{remote}"
            );
        }
    }

    #[test]
    fn a_credential_in_the_remote_is_never_in_the_page() {
        let made = page(
            "https://ada:ghp_SECRET@github.com/ada/engine.git",
            "main",
            "x",
        );
        assert_eq!(
            made.as_deref(),
            Some("https://github.com/ada/engine/compare/main...x?expand=1")
        );
    }

    #[test]
    fn a_remote_that_is_not_a_known_forge_or_has_odd_parts_gives_no_page() {
        for remote in [
            "https://example.com/ada/engine.git",
            "/home/ada/engine.git",
            "file:///home/ada/engine.git",
            "https://github.com/ada/../engine",
            "https://github.com/ada/engine?x=1",
            "https://github.com/ada",
            "ext::sh -c evil",
        ] {
            assert_eq!(page(remote, "main", "x"), None, "{remote}");
        }
    }

    #[test]
    fn a_gitlab_remote_gives_the_new_merge_request_page() {
        let made = page("git@gitlab.com:group/sub/engine.git", "main", "fix/a b").expect("a page");
        assert!(made.starts_with("https://gitlab.com/group/sub/engine/-/merge_requests/new?"));
        assert!(made.contains("source_branch%5D=fix/a%20b"));
    }
}
