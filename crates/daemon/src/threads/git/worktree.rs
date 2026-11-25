//! The worktree of a thread (TH41): made from a repository of the person without a change of
//! its checkout, used again when it is there, and removed only after its work is saved to a
//! private ref (THR-097).
//!
//! No `git` runs with the Git folder of the person: the branch and the entry of the worktree
//! are written there as files ([`refs`], [`linked`]), and the files are checked out with the
//! Git folder of the runner ([`own`]). So no hook and no setting that somebody left in the
//! repository applies when a worktree is made or removed (THR-050).

pub mod files;
mod removal;
mod retirement;
mod undo;

pub use removal::{remove, remove_broken};

pub use undo::Pending;
pub(super) use undo::Undo;

use std::path::{Path, PathBuf};

use agent_threads::git::{GitRefusal, Preparing};
use uuid::Uuid;

use super::place::{self, Home, Record, Worktree, own, shared};
use super::project::refs::{self, Was};
use super::project::{self, Layout, linked};
use super::run::folder::FolderId;
use super::run::{Git, GitError, Run};
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
    let (made, undo) = undoable(git, home, plan, shown).await?;
    if let Some(undo) = undo {
        undo.keep();
    }
    Ok(made)
}

/// As [`make`], with what the call made: the caller keeps it, or drops it and it goes again.
/// `None` for a worktree that was there already.
///
/// # Errors
/// As [`make`].
pub(super) async fn undoable(
    git: &Git,
    home: &Home,
    plan: Plan<'_>,
    shown: Shown<'_>,
) -> Result<(Made, Option<Undo>), GitRefusal> {
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
            Ok((made(worktree, true), None))
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
    // A Git folder of the runner that is there with no record (a record that the system
    // removed) holds checkpoints of an earlier worktree: an undo never removes it.
    let own_made = !home.private_git_exists(&own).map_err(GitError::Files)?;
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
        home: home.clone(),
        thread: plan.thread,
        repository: repository.clone(),
        own: own_made.then(|| own.clone()),
        folder: folder.clone(),
        entry: None,
        branch: None,
        at: at.clone(),
        kept: false,
    };
    home.state_parent(&own, true).map_err(GitError::Files)?;
    home.state_parent(&shared, true).map_err(GitError::Files)?;
    home.private_git_exists(&shared).map_err(GitError::Files)?;
    shared::ensure(git, (&shared, &project), at.len() == 64).await?;
    own::make(git, &own, &shared, at.len() == 64).await?;
    if existing.is_none() {
        refs::put(common, &full, &at, Was::Absent)?;
        undo.branch = Some((common.to_owned(), full.clone()));
    }
    let entry = format!("anima-{}", Uuid::now_v7().as_simple());
    let layout = linked::add(common, &entry, &folder, &branch)?;
    undo.entry = Some(layout.clone());
    let target = (plan.thread, repository.as_str());
    let worktree = filled((git, home), target, (&layout, new), shown).await?;
    Ok((made(worktree, false), Some(undo)))
}

impl super::Engine {
    /// Makes or finds the worktree of a thread. The runner calls it when the person starts a
    /// thread in a worktree; it is not an action of a client.
    ///
    /// # Errors
    /// As [`make`].
    pub async fn worktree(&self, plan: Plan<'_>) -> Result<Made, GitRefusal> {
        self.worktree_shown(plan, None).await
    }

    /// As [`Engine::worktree`]; `shown` hears each step, for the page. Dropping the call
    /// cancels it: what it made so far goes again.
    ///
    /// # Errors
    /// As [`make`].
    pub async fn worktree_shown(
        &self,
        plan: Plan<'_>,
        shown: Shown<'_>,
    ) -> Result<Made, GitRefusal> {
        self.worktree_pending(plan, shown).await.map(Pending::keep)
    }

    /// As [`Engine::worktree_shown`], before the start of the thread: the runner keeps the
    /// worktree when the start passed, or drops it and what this call made goes again, with
    /// nothing saved ([`Pending`]).
    ///
    /// # Errors
    /// As [`make`].
    pub async fn worktree_pending(
        &self,
        plan: Plan<'_>,
        shown: Shown<'_>,
    ) -> Result<Pending, GitRefusal> {
        self.closed()?;
        // One worktree of a thread is made at a time, whatever its repository; and no action
        // of the person runs on this worktree before it is kept or gone again ([`Pending`]).
        let real = std::fs::canonicalize(plan.project);
        let name = real.as_deref().ok().and_then(Path::file_name);
        let repository = place::folder_name(&name.unwrap_or_default().to_string_lossy());
        let alone = [
            self.alone(plan.thread, "").await,
            self.alone(plan.thread, &repository).await,
        ];
        // One worktree of a project at a time: the first one copies its history, and the
        // next one finds the copy.
        let project = real.as_deref().unwrap_or(plan.project).to_string_lossy();
        let _copy = self.alone(Uuid::nil(), &project).await;
        self.closed()?;
        let (made, undo) = undoable(&self.git, &self.home, plan, shown).await?;
        Ok(Pending::new(made, undo, alone))
    }
}

/// Where the worktree of `repository` of `thread` will be, when nothing is there and the
/// runner made none before.
fn free_place(home: &Home, thread: Uuid, repository: &str) -> Result<PathBuf, GitRefusal> {
    if home
        .active(thread, repository)
        .map_err(|error| GitRefusal::Failed {
            said: format!("Could not check for an existing worktree record: {error}"),
        })?
        .is_some()
    {
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
        trusted: trust::baseline(home, new.project)?,
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
