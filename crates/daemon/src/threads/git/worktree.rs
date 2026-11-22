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
    let folder = home.planned(plan.thread, &repository)?;
    if folder.symlink_metadata().is_ok() {
        // Something is at the place and it is not a worktree of the runner: it is not touched.
        return Err(GitRefusal::BadPath);
    }
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
/// # Errors
/// A [`GitRefusal`] when the work could not be saved (nothing is removed then) or a step
/// fails.
pub async fn remove(git: &Git, home: &Home, worktree: &Worktree) -> Result<String, GitRefusal> {
    let held = &git.for_worktree(worktree).await?;
    let layout = worktree.layout();
    let record = worktree.record();
    let kept = checkpoint::kept_ref(worktree.thread());
    let commit = checkpoint::snapshot(held, worktree).await?;
    let tip = own::tip(held, worktree).await?.commit;
    own::give(held, worktree, &commit, tip.as_deref()).await?;
    refs::put(&layout.common_dir, &kept, &commit, Was::Any)?;
    // The folder is still the one that the runner made, not one put in its place (P12).
    if record.folder_id.is_some() && FolderId::of(&record.folder) != record.folder_id {
        return Err(GitRefusal::NoWorktree);
    }
    let gone = |failed: std::io::Error| GitRefusal::Failed {
        said: format!("the worktree could not be removed: {failed}"),
    };
    std::fs::remove_dir_all(&record.folder).map_err(gone)?;
    linked::remove(&layout)?;
    std::fs::remove_dir_all(&record.own).map_err(gone)?;
    if let Some(repository) = worktree.folder().file_name().and_then(OsStr::to_str) {
        home.forget(worktree.thread(), repository);
    }
    // The folder of the thread goes when it is empty; one with a second repository stays.
    let _ = record.folder.parent().map(std::fs::remove_dir);
    Ok(kept)
}
