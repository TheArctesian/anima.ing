//! The worktree of a thread (TH41): made from a repository of the person without a change of
//! its checkout, used again when it is there, and removed only after its work is saved to a
//! private ref (THR-097).
//!
//! No `git` runs with the Git folder of the person: the branch and the entry of the worktree
//! are written there as files ([`refs`], [`linked`]), and the files are checked out with the
//! Git folder of the runner ([`own`]). So no hook and no setting that somebody left in the
//! repository applies when a worktree is made or removed (THR-050).

pub mod files;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use agent_threads::git::{GitRefusal, Preparing};
use uuid::Uuid;

use super::place::{self, Home, Record, Worktree, own, shared};
use super::project::refs::{self, Was};
use super::project::{self, Layout, linked};
use super::run::folder::FolderId;
use super::run::{Git, Run};
use super::{checkpoint, trust};

/// A worktree that is ready for a thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    pub worktree: Worktree,
    /// The name of the repository, which is the name of the folder of the worktree.
    pub repository: String,
    pub branch: String,
    /// The worktree was there already; nothing was made.
    pub reused: bool,
}

/// What to make a worktree from.
#[derive(Clone, Copy, Debug)]
pub struct Plan<'a> {
    pub thread: Uuid,
    /// The repository of the person: the top folder of a checkout.
    pub project: &'a Path,
    /// The branch to start from; `HEAD` of the project when `None`.
    pub base: Option<&'a str>,
}

/// The real top folder of the repository at `project`, which must not be in the home of the
/// worktrees, with its Git folders.
fn repository(home: &Home, project: &Path) -> Result<(PathBuf, Layout), GitRefusal> {
    if !project.is_absolute() {
        return Err(GitRefusal::BadPath);
    }
    let real = std::fs::canonicalize(project).map_err(|_| GitRefusal::NotARepository)?;
    if std::fs::canonicalize(home.folder()).is_ok_and(|home| real.starts_with(home)) {
        return Err(GitRefusal::BadPath);
    }
    let layout = project::layout(&real)?;
    Ok((real, layout))
}

/// What the runner is doing while it makes a worktree, for the page to show.
pub type Shown<'a> = Option<&'a (dyn Fn(Preparing) + Sync)>;

/// Makes the worktree of `plan.thread` for the repository at `plan.project`, or gives the one
/// that is there. The checkout of the person is not switched and no file of it changes.
/// `shown` hears each step. When the call is dropped before its end (the person cancels the
/// copy of a large history), what it made goes again.
///
/// # Errors
/// A [`GitRefusal`] when the project is not a repository, the base is unknown, the branch name
/// is taken by something the runner did not make, or a step fails.
pub async fn make(
    git: &Git,
    home: &Home,
    plan: Plan<'_>,
    shown: Shown<'_>,
) -> Result<Made, GitRefusal> {
    let (project, layout) = repository(home, plan.project)?;
    let name = project.file_name().map(|name| name.to_string_lossy());
    let repository = place::folder_name(name.as_deref().unwrap_or_default());
    let branch = place::thread_branch(plan.thread);
    let made = |worktree, reused| Made {
        worktree,
        repository: repository.clone(),
        branch: branch.clone(),
        reused,
    };
    if let Ok(worktree) = home.worktree(plan.thread, &repository) {
        // A worktree of the same name from a different repository is not this one.
        return if worktree.record().project == project {
            Ok(made(worktree, true))
        } else {
            Err(GitRefusal::BadPath)
        };
    }
    let folder = free_place(home, plan.thread, &repository)?;
    let common = layout.common_dir.as_path();
    let full = format!("refs/heads/{branch}");
    let (existing, at) = branch_at(&layout, plan, &full)?;
    place::private_folder(&home.of_thread(plan.thread)).map_err(|_| GitRefusal::BadPath)?;
    let own = home.own(plan.thread, &repository)?;
    let shared = home.shared(&project);
    let new = New {
        project: &project,
        folder: &folder,
        own: &own,
        branch: &branch,
        at: &at,
    };
    // From here each thing that this call makes goes again unless the call comes to its end.
    let mut undo = Undo {
        home,
        target: (plan.thread, repository.as_str()),
        new,
        entry: None,
        branch: None,
        kept: false,
    };
    shared::ensure(git, (&shared, &project), at.len() == 64).await?;
    own::make(git, &own, &shared, at.len() == 64).await?;
    if existing.is_none() {
        refs::put(common, &full, &at, Was::Absent)?;
        undo.branch = Some((common, full.as_str()));
    }
    let entry = format!("anima-{}", plan.thread.as_simple());
    let layout = linked::add(common, &entry, &folder, &branch)?;
    undo.entry = Some(layout.clone());
    let worktree = filled((git, home), undo.target, (&layout, new), shown).await?;
    undo.kept = true;
    Ok(made(worktree, false))
}

/// Where the worktree of `repository` of `thread` will be, when nothing is there and the
/// runner made none before.
fn free_place(home: &Home, thread: Uuid, repository: &str) -> Result<PathBuf, GitRefusal> {
    if home.recorded(thread, repository).is_ok() {
        // The runner made this worktree before and it is not whole now (its folder is gone or
        // was replaced). Its checkpoints are in the folder of the runner: nothing is made
        // new over them.
        return Err(GitRefusal::Failed {
            said: "the worktree of this chat is gone or was replaced; its checkpoints are \
                   kept. Remove the worktree of the chat before a new one is made"
                .to_owned(),
        });
    }
    let folder = home.planned(thread, repository)?;
    if folder.symlink_metadata().is_ok() {
        // Something is at the place and it is not a worktree of the runner: it is not touched.
        return Err(GitRefusal::BadPath);
    }
    Ok(folder)
}

/// Whether the branch `full` of the thread is there already, and the commit that the worktree
/// starts at: that branch, or the base of the plan.
fn branch_at(
    layout: &Layout,
    plan: Plan<'_>,
    full: &str,
) -> Result<(Option<String>, String), GitRefusal> {
    let common = layout.common_dir.as_path();
    let start = start_of(layout, plan.base)?;
    let existing = refs::resolve(common, full)?;
    // A branch of this name that the runner did not keep for this thread is never taken over.
    let kept = checkpoint::kept_ref(plan.thread);
    if existing.is_some() && refs::resolve(common, &kept)?.is_none() {
        return Err(GitRefusal::BranchExists);
    }
    let at = existing.clone().unwrap_or(start);
    Ok((existing, at))
}

/// The parts of a worktree that is being made.
#[derive(Clone, Copy)]
struct New<'a> {
    project: &'a Path,
    folder: &'a Path,
    own: &'a Path,
    branch: &'a str,
    /// The commit that the branch starts at.
    at: &'a str,
}

/// What a call that makes a worktree made so far. When it is dropped before the worktree is
/// whole (a step failed, or the call was cancelled), each of them goes again: they hold no
/// work of a thread.
struct Undo<'a> {
    home: &'a Home,
    target: (Uuid, &'a str),
    new: New<'a>,
    /// The entry of the worktree in the Git folder of the person.
    entry: Option<Layout>,
    /// The branch that this call made: the shared Git folder and the name of its ref.
    branch: Option<(&'a Path, &'a str)>,
    kept: bool,
}

impl Drop for Undo<'_> {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        let _ = std::fs::remove_dir_all(self.new.own);
        self.home.forget(self.target.0, self.target.1);
        // The folder was not there when the call started: what is there now is of this call.
        let _ = std::fs::remove_dir_all(self.new.folder);
        if let Some(entry) = &self.entry {
            let _ = linked::remove(entry);
        }
        if let Some((common, full)) = self.branch {
            let _ = refs::take_back(common, full, self.new.at);
        }
    }
}

/// Writes the record of a worktree whose entry was made a moment ago, copies the history of
/// its start, checks out its files and takes the checkpoint of the start.
async fn filled(
    (git, home): (&Git, &Home),
    (thread, repository): (Uuid, &str),
    (layout, new): (&Layout, New<'_>),
    shown: Shown<'_>,
) -> Result<Worktree, GitRefusal> {
    let say = |step| {
        if let Some(shown) = shown {
            shown(step);
        }
    };
    // The record is written now, before an agent runs in the worktree: each later run of
    // `git` is held to these places, whatever the files of the worktree say then.
    let folder = std::fs::canonicalize(new.folder).map_err(|_| GitRefusal::NoWorktree)?;
    let record = Record {
        project: new.project.to_owned(),
        home: home.folder().to_owned(),
        folder_id: FolderId::of(&folder),
        folder,
        git_dir: layout.git_dir.clone(),
        common_dir: layout.common_dir.clone(),
        own: new.own.to_owned(),
        shared: home.shared(new.project),
        branches: vec![new.branch.to_owned()],
        // Only what the person accepted for this repository before counts as accepted: an
        // earlier agent can have changed the remote of the repository, which each worktree
        // shares.
        trusted: trust::baseline(home, new.project),
    };
    home.keep(thread, repository, &record)?;
    let worktree = home.worktree(thread, repository)?;
    let git = &git.for_worktree(&worktree).await?;
    say(Preparing::History);
    shared::learn(git, &worktree, &[new.at]).await?;
    say(Preparing::Files);
    let files = ["reset", "--hard", "--quiet", new.at, "--"];
    git.output(Run::new(worktree.folder(), files)).await?;
    own::follow(git, &worktree, new.at).await?;
    checkpoint::capture(git, &worktree, 0).await?;
    Ok(worktree)
}

/// The commit to start from: the branch `base` when there is one (a branch of the repository,
/// else one of a remote), else `HEAD` of the project.
fn start_of(layout: &Layout, base: Option<&str>) -> Result<String, GitRefusal> {
    let common = layout.common_dir.as_path();
    let found = match base {
        Some(base) => {
            let base = place::branch_name(base)?;
            match refs::resolve(common, &format!("refs/heads/{base}"))? {
                Some(found) => Some(found),
                None => refs::resolve(common, &format!("refs/remotes/{base}"))?,
            }
        }
        None => refs::head(layout)?.commit,
    };
    found.ok_or(GitRefusal::UnknownBase)
}

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
    let commit = checkpoint::snapshot_on(held, worktree, tip.clone()).await?;
    own::give(held, worktree, &commit, tip.as_deref()).await?;
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
    // What is left of a folder whose work is saved goes too, when it is still the folder
    // that the runner made at the place that the runner planned.
    let planned = Home::at(&record.home, home.records())?.planned(thread, repository)?;
    let ours = std::fs::canonicalize(&planned).is_ok_and(|real| real == record.folder)
        && (record.folder_id.is_none() || FolderId::of(&record.folder) == record.folder_id);
    if saved && ours {
        std::fs::remove_dir_all(&record.folder).map_err(|failed| gone(&failed))?;
    }
    cleared(home, thread, &record)?;
    Ok(saved.then(|| checkpoint::kept_ref(thread)))
}
