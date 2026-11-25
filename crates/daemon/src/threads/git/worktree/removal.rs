//! Retires a worktree after saving its Git snapshot to a private ref (THR-097).

use agent_threads::git::{GitOutcome, GitRefusal};
use uuid::Uuid;

use super::super::checkpoint;
use super::super::place::{Home, Worktree, own, shared};
use super::super::project::refs;
use super::super::run::Git;
use super::retirement::{Completion, Retirement};

/// Saves a private ref before moving the whole original directory out of use. Files that
/// cannot go in a Git snapshot stay in the original directory, as do late writes to it.
/// The branch and linked Git metadata stay for recovery.
///
/// # Errors
/// The reason a snapshot, move, or active record update could not be completed.
pub async fn remove(git: &Git, home: &Home, worktree: &Worktree) -> Result<GitOutcome, GitRefusal> {
    let record = worktree.record();
    let repository = record
        .folder
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or(GitRefusal::BadPath)?;
    let target = (worktree.thread(), repository);
    let retirement = if let Some(pending) = Retirement::existing(record)? {
        pending
    } else {
        let kept = save(git, worktree).await?;
        Retirement::prepare(record, kept)?
    };
    let (kept, recovery) = retirement.finish(home, target, record)?;
    Ok(removed(kept, recovery, record.folder.clone()))
}

async fn save(git: &Git, worktree: &Worktree) -> Result<String, GitRefusal> {
    let record = worktree.record();
    let common = &record.common_dir;
    let kept = checkpoint::kept_ref(worktree.thread());
    let held = &git.for_worktree(worktree).await?.with_own_room(u64::MAX);
    // If the old tip cannot be read, the saved commit can hold the files without a parent.
    let tip = match own::tip(held, worktree).await {
        Ok(at) => at.commit,
        Err(_) => made_branch(held, worktree).await,
    };
    let commit = checkpoint::snapshot_on(held, worktree, tip.clone()).await?;
    own::give(held, worktree, &commit, tip.as_deref()).await?;
    if let Ok(Some(earlier)) = refs::resolve(common, &kept) {
        let aside = format!("refs/anima/kept-before/{}/{earlier}", worktree.thread());
        refs::own_put(common, &aside, &earlier)?;
    }
    refs::own_put(common, &kept, &commit)?;
    Ok(kept)
}

/// The last commit of the branch that the runner made for the thread, when the thread can
/// bring it into the copy of the project.
async fn made_branch(git: &Git, worktree: &Worktree) -> Option<String> {
    let record = worktree.record();
    let branch = format!("refs/heads/{}", record.branches.first()?);
    let commit = refs::resolve(&record.common_dir, &branch).ok()??;
    let known = shared::learn(git, worktree, &[&commit]).await;
    known.is_ok().then_some(commit)
}

/// Finishes a recorded move or saves the original directory of a broken linked worktree.
///
/// # Errors
/// The reason a recorded move or worktree could not be recovered.
pub async fn remove_broken(
    git: &Git,
    home: &Home,
    target: (Uuid, &str),
) -> Result<GitOutcome, GitRefusal> {
    let (thread, repository) = target;
    let record = match home.recorded(thread, repository) {
        Ok(record) => record,
        Err(error) => {
            let path = home.record_file(thread, repository)?;
            if matches!(path.symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
            {
                return Completion::get(home, target);
            }
            return Err(error.into());
        }
    };
    if let Some(pending) = Retirement::existing(&record)? {
        let (kept, recovery) = pending.finish(home, target, &record)?;
        return Ok(removed(kept, recovery, record.folder.clone()));
    }
    if let Some(worktree) = home.for_removal(thread, repository) {
        remove(git, home, &worktree).await
    } else {
        Completion::orphan(home, target, &record)
    }
}

fn removed(kept: String, recovery: std::path::PathBuf, original: std::path::PathBuf) -> GitOutcome {
    GitOutcome::Removed {
        kept: Some(kept),
        recovery: Some(recovery),
        original: Some(original),
    }
}
