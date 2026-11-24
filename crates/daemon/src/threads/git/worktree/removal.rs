//! The removal of the worktree of a thread: its work is saved to a private ref of the thread
//! in the repository of the person first (THR-097), then the folder, its entry and the Git
//! folder of the runner go.

use std::ffi::OsStr;

use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::super::checkpoint;
use super::super::place::{Home, Record, Worktree, own, shared};
use super::super::project::{Layout, linked, refs};
use super::super::run::Git;
use super::super::run::folder::FolderId;

/// Saves the files of the worktree to the private ref of its thread in the repository of the
/// person, then removes the worktree, its entry and the Git folder of the runner with the
/// checkpoints of the thread. The branch stays. Gives the name of the ref.
///
/// The work is saved one time. A removal that stops in the middle (a folder that cannot be
/// emptied) leaves a note that the work is saved; the next try does not save again what is
/// left of the files over the whole work.
///
/// # Errors
/// A [`GitRefusal`] when the work could not be saved (nothing is removed then) or a step
/// fails.
pub async fn remove(git: &Git, home: &Home, worktree: &Worktree) -> Result<String, GitRefusal> {
    let record = worktree.record();
    let common = &record.common_dir;
    let kept = checkpoint::kept_ref(worktree.thread());
    let started = record.own.join(own::SAVED).exists();
    // The work is saved whatever the size of the checkpoints of the thread is: it goes to
    // the repository of the person, and the folder of the runner is removed after.
    let held = &git.for_worktree(worktree).await?.with_own_room(u64::MAX);
    // A last commit that cannot be brought (the copy of the project is full, or the branch
    // names a commit that is not there) does not keep the worktree: the saved commit then
    // holds each file and has no parent.
    let tip = match own::tip(held, worktree).await {
        Ok(at) => at.commit,
        // The place of the worktree cannot be read: the branch that the runner made for the
        // thread says what the last commit is, when the thread can bring it.
        Err(_) => made_branch(held, worktree).await,
    };
    let (commit, preserved) = checkpoint::snapshot_for_removal(held, worktree, tip.clone()).await?;
    own::give(held, worktree, &commit, tip.as_deref()).await?;
    // Packing the saved work can take time. Check for paths that appeared while it ran.
    preserved.check(worktree.folder())?;
    if started {
        // A removal that stopped in the middle saved the whole work. What is left of the
        // files now, with what changed since, is saved beside it and never over it.
        let rest = format!("refs/anima/kept-rest/{}/{commit}", worktree.thread());
        refs::own_put(common, &rest, &commit)?;
    } else {
        // The work that an earlier removal of this thread saved stays under a name of its own.
        // A ref there that is no ref of the runner (an agent can write it) names no work.
        if let Ok(Some(earlier)) = refs::resolve(common, &kept) {
            let aside = format!("refs/anima/kept-before/{}/{earlier}", worktree.thread());
            refs::own_put(common, &aside, &earlier)?;
        }
        refs::own_put(common, &kept, &commit)?;
        std::fs::write(record.own.join(own::SAVED), &commit).map_err(|failed| gone(&failed))?;
    }
    // The folder is still the one that the runner made, not one put in its place (P12).
    if record.folder_id.is_some() && FolderId::of(&record.folder) != record.folder_id {
        return Err(GitRefusal::NoWorktree);
    }
    std::fs::remove_dir_all(&record.folder).map_err(|failed| gone(&failed))?;
    cleared(home, worktree.thread(), record)?;
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

fn gone(failed: &std::io::Error) -> GitRefusal {
    GitRefusal::Failed {
        said: format!("the worktree could not be removed: {failed}"),
    }
}

/// Removes what the runner keeps for a worktree whose folder is gone or is not removed by
/// the runner: its entry in the repository of the person, the Git folder of the runner with
/// the checkpoints, and the record.
fn cleared(home: &Home, thread: Uuid, record: &Record) -> Result<(), GitRefusal> {
    let layout = Layout {
        git_dir: record.git_dir.clone(),
        common_dir: record.common_dir.clone(),
    };
    // An entry that is gone already, with its Git folder, is the wanted state.
    if record.common_dir.is_dir() {
        linked::remove(&layout)?;
    }
    match std::fs::remove_dir_all(&record.own) {
        Err(failed) if failed.kind() != std::io::ErrorKind::NotFound => {
            return Err(gone(&failed));
        }
        _ => {}
    }
    if let Some(repository) = record.folder.file_name().and_then(OsStr::to_str) {
        home.forget(thread, repository);
    }
    // The folder of the thread goes when it is empty; one with a second repository stays.
    let _ = record.folder.parent().map(std::fs::remove_dir);
    Ok(())
}

/// Removes a worktree that is not whole any more (its `.git` file or its entry is gone, or an
/// earlier removal stopped in the middle), so that the thread and its project are never held
/// by it. When its folder is still the one that the runner made, the work is saved first, as
/// in [`remove`]. When the work cannot be saved and was not saved before, no file of the
/// folder is touched: the person can still take them; the answer then names no saved work.
///
/// # Errors
/// A [`GitRefusal`] when the runner has no record of the worktree, or a step fails.
pub async fn remove_broken(
    git: &Git,
    home: &Home,
    (thread, repository): (Uuid, &str),
) -> Result<Option<String>, GitRefusal> {
    let record = home.recorded(thread, repository)?;
    // A folder that is still the one of the runner is saved as in each removal. When that
    // fails, nothing goes: the failure is the answer, and the next try saves again.
    if let Some(worktree) = home.for_removal(thread, repository) {
        return remove(git, home, &worktree).await.map(Some);
    }
    let saved = record.own.join(own::SAVED).exists();
    // A prior save does not account for files added after that save.
    let planned = Home::at(&record.home, home.records())?.planned(thread, repository)?;
    let ours = std::fs::canonicalize(&planned).is_ok_and(|real| real == record.folder)
        && (record.folder_id.is_none() || FolderId::of(&record.folder) == record.folder_id);
    if saved && ours {
        return Err(GitRefusal::Failed {
            said: "The remaining worktree files could not be checked. Move the worktree to another folder before retrying removal.".to_owned(),
        });
    }
    cleared(home, thread, &record)?;
    Ok(saved.then(|| checkpoint::kept_ref(thread)))
}
