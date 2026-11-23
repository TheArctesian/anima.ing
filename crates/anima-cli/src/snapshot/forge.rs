//! Every ref sent starts with `PREFIX`; nothing is force-pushed and no branch or tag is touched (JOB-017).

use super::SnapshotError;
use super::git::Git;
use super::git_output::{RemoteScratchRef, rejection, safe, scratch_refs};
use super::object_paths::changed_paths;
use super::remote_name;
use super::scratch_ref::{PREFIX, ScratchRef};
use agent_threads::files::secret_paths;

/// Only a name that `git remote` lists passes, never a URL or an option.
///
/// # Errors
/// [`SnapshotError::BadRemote`] or [`SnapshotError::UnknownRemote`].
pub fn remote<'a>(git: &Git, name: &'a str) -> Result<&'a str, SnapshotError> {
    let name = remote_name::checked(name)?;
    let names = git.text(&["remote"])?;
    if names.lines().any(|known| known == name) {
        Ok(name)
    } else {
        Err(SnapshotError::UnknownRemote {
            name: name.to_owned(),
        })
    }
}

/// Pushes with no `+`, no tags and no submodules, and refuses when an object it could send has a
/// path that looks like a secret.
///
/// # Errors
/// [`SnapshotError::SecretHistory`], [`SnapshotError::Rejected`] or [`SnapshotError::Unreachable`].
pub fn push(
    git: &Git,
    remote: &str,
    commit: &str,
    scratch: &ScratchRef,
) -> Result<(), SnapshotError> {
    refuse_unsent_secrets(git, remote, commit)?;
    let refspec = format!("{commit}:{}", scratch.as_str());
    send(git, remote, scratch, &refspec)
}

/// Checks each path added or changed outside the remote-tracking history, also one a later commit deletes.
fn refuse_unsent_secrets(git: &Git, remote: &str, commit: &str) -> Result<(), SnapshotError> {
    let remotes = format!("--remotes={remote}");
    let listed = git.bytes(&[
        "log",
        "--format=",
        "--no-notes",
        "--no-show-signature",
        "--name-only",
        "--diff-filter=AMT",
        "--no-renames",
        "--diff-merges=separate",
        "--root",
        "-z",
        commit,
        "--not",
        &remotes,
        "--",
    ])?;
    let all = changed_paths(&listed);
    let mut found = secret_paths(all.iter().map(String::as_str));
    if found.is_empty() {
        return Ok(());
    }
    found.sort();
    found.dedup();
    Err(SnapshotError::SecretHistory {
        remote: remote.to_owned(),
        paths: found,
    })
}

/// # Errors
/// [`SnapshotError::Unreachable`] when `git ls-remote` fails.
pub fn list(git: &Git, remote: &str) -> Result<Vec<RemoteScratchRef>, SnapshotError> {
    let pattern = format!("{PREFIX}*");
    let ran = git.run(&["ls-remote", "--refs", remote, &pattern])?;
    if !ran.success {
        return Err(SnapshotError::Unreachable {
            remote: remote.to_owned(),
            reason: safe(&ran.stderr),
        });
    }
    Ok(scratch_refs(&String::from_utf8_lossy(&ran.stdout)))
}

/// # Errors
/// [`SnapshotError::NoSuchRef`] when the remote has no such ref; as [`list`].
pub fn find(
    git: &Git,
    remote: &str,
    scratch: &ScratchRef,
) -> Result<RemoteScratchRef, SnapshotError> {
    list(git, remote)?
        .into_iter()
        .find(|row| row.scratch == *scratch)
        .ok_or_else(|| SnapshotError::NoSuchRef {
            remote: remote.to_owned(),
            scratch: scratch.as_str().to_owned(),
        })
}

/// # Errors
/// As [`push`].
pub fn delete(git: &Git, remote: &str, scratch: &ScratchRef) -> Result<(), SnapshotError> {
    let refspec = format!(":{}", scratch.as_str());
    send(git, remote, scratch, &refspec)
}

fn send(git: &Git, remote: &str, scratch: &ScratchRef, refspec: &str) -> Result<(), SnapshotError> {
    let ran = git.run(&[
        "push",
        "--porcelain",
        "--no-follow-tags",
        "--recurse-submodules=no",
        remote,
        refspec,
    ])?;
    let stdout = String::from_utf8_lossy(&ran.stdout);
    if let Some(reason) = rejection(&stdout) {
        return Err(SnapshotError::Rejected {
            remote: remote.to_owned(),
            scratch: scratch.as_str().to_owned(),
            reason,
        });
    }
    if ran.success {
        Ok(())
    } else {
        Err(SnapshotError::Unreachable {
            remote: remote.to_owned(),
            reason: safe(&ran.stderr),
        })
    }
}
