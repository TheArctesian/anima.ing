//! The revert of the files of a worktree to a checkpoint (TH44, the file part of "Edit from
//! here"). It takes a [`Worktree`], so it cannot run in the checkout of the person (P12).
//!
//! I/O: `git restore`, `git clean` and `git reset`, each held to the folder of the worktree.

use agent_threads::git::GitRefusal;

use super::checkpoint;
use super::place::Worktree;
use super::run::{Git, GitError, Run};

/// Puts the files back as they were at the checkpoint of `turn`. The files as they were before
/// are kept under a ref first, whose name is the answer, so that the revert can be undone
/// (THR-099). The checkpoints after `turn` are deleted. The branch does not move.
///
/// # Errors
/// [`GitRefusal::UnknownCheckpoint`] when the turn has none; a refusal with the words of `git`
/// when a step fails, after the files were put back as they were before.
pub async fn to_turn(git: &Git, worktree: &Worktree, turn: u32) -> Result<String, GitRefusal> {
    let target = checkpoint::list(git, worktree)
        .await?
        .into_iter()
        .find(|checkpoint| checkpoint.turn == turn)
        .ok_or(GitRefusal::UnknownCheckpoint)?;
    let undo = checkpoint::before_revert_ref(worktree.thread());
    let before = checkpoint::snapshot(git, worktree).await?;
    checkpoint::keep(git, worktree, &undo, &before).await?;
    if let Err(failed) = files_as_at(git, worktree, &target.commit).await {
        // The files go back to the snapshot of a moment ago; the first failure is the answer.
        let _ = files_as_at(git, worktree, &before).await;
        return Err(failed.into());
    }
    checkpoint::forget(git, worktree, |other| other > turn).await?;
    Ok(undo)
}

/// Makes the files of the worktree equal to the tree of `commit`: restores each file of it,
/// removes each file that is not in it and that git does not ignore, and puts the index back to
/// the branch.
async fn files_as_at(git: &Git, worktree: &Worktree, commit: &str) -> Result<(), GitError> {
    let dir = worktree.folder();
    let restore = [
        "restore",
        "--source",
        commit,
        "--worktree",
        "--staged",
        "--",
        ".",
    ];
    git.output(Run::new(dir, restore)).await?;
    git.output(Run::new(dir, ["clean", "-f", "-d", "--", "."]))
        .await?;
    match git
        .output(Run::new(dir, ["reset", "--quiet", "--", "."]))
        .await
    {
        // A repository with no commit has no `HEAD` to reset to; the files are right already.
        Ok(_) | Err(GitError::Failed { .. }) => Ok(()),
        Err(other) => Err(other),
    }
}
