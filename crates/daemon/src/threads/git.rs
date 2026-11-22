//! The Git of a thread (stage 7 of the copy of T3 Code, tasks TH41 to TH45): a worktree for
//! each thread, a checkpoint at each end of a turn, the changes, the revert, and the commit, the
//! push and the pull request that the person asks for.
//!
//! [`Engine::call`] is the one door for a client. It refuses an agent each action that changes
//! something (THR-049), and it finds the worktree by the ID of the thread alone, so that no
//! path of a client reaches `git` and the checkout of the person is never changed (P12).
//!
//! Each `git` of the runner runs with a Git folder of the runner ([`place::own`]), never with
//! the one of the repository of the person: an agent can write the configuration and the
//! hooks there at any moment, and no list of names turns off what is written a moment later
//! (THR-050). What the repository of the person must give or get goes through [`project`], as
//! files.
//!
//! Pure: [`place`] (the checks), the readers of [`changes`] and [`push::pull_request`].
//! I/O: [`run`] starts `git`; [`project`] reads and writes files of the person's Git folder.

pub mod changes;
pub mod checkpoint;
pub mod commit;
pub mod place;
pub mod project;
pub mod push;
pub mod revert;
pub mod run;
pub mod status;
#[cfg(test)]
mod tests;
pub mod trust;
pub mod worktree;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use agent_threads::git::{Changes, GitAction, GitOutcome, GitRefusal, Scope};
use uuid::Uuid;

use place::{BadPlace, Home, Record, Worktree};
use project::ProjectError;
use push::pull_request;
use run::settings::Setting;
use run::{Git, GitError};

impl From<ProjectError> for GitRefusal {
    fn from(error: ProjectError) -> Self {
        match error {
            ProjectError::NotARepository => Self::NotARepository,
            other => Self::Failed {
                said: other.to_string(),
            },
        }
    }
}

impl From<GitError> for GitRefusal {
    fn from(error: GitError) -> Self {
        match error {
            GitError::NotFound => Self::GitNotFound,
            other => Self::Failed {
                said: other.to_string(),
            },
        }
    }
}

impl From<BadPlace> for GitRefusal {
    fn from(bad: BadPlace) -> Self {
        match bad {
            BadPlace::NotOurWorktree | BadPlace::MainCheckout => Self::NoWorktree,
            BadPlace::Escapes | BadPlace::BadName => Self::BadPath,
        }
    }
}

/// Who calls, as the runner saw it: the runner decides both, never the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caller {
    /// The call came from a process of an agent (`runner::peer::of_an_agent`).
    pub agent: bool,
    /// A turn of the thread runs now.
    pub running: bool,
    /// The thread whose agent calls; `None` for the person. An agent reads only its own thread.
    pub thread: Option<Uuid>,
}

/// One worktree of a thread, named by the thread and the repository, never by a path.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    pub thread: Uuid,
    pub repository: &'a str,
}

/// The locks of the worktrees, by thread and the name of the repository: one for the actions
/// that change something, one (with `/index` after the name) for each use of the index of the
/// runner.
type Locks = HashMap<(Uuid, String), Arc<tokio::sync::Mutex<()>>>;

/// The Git of the threads of this computer. A clone is cheap and is the same engine: the
/// runner gives one to each task that does a call, and a call can take minutes (a hook).
#[derive(Clone, Debug)]
pub struct Engine {
    git: Git,
    home: Home,
    /// One lock for each worktree: a commit of the person and the checkpoint of a turn that
    /// ended never change one worktree at the same time.
    locks: Arc<Mutex<Locks>>,
}

impl Engine {
    #[must_use]
    pub fn new(git: Git, home: Home) -> Self {
        Self {
            git,
            home,
            locks: Arc::default(),
        }
    }

    /// Waits until no other task changes the worktree of `repository` of `thread`.
    async fn alone(&self, thread: Uuid, repository: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
            let key = (thread, repository.to_owned());
            Arc::clone(locks.entry(key).or_default())
        };
        lock.lock_owned().await
    }

    /// Makes or finds the worktree of a thread. The runner calls it when the person starts a
    /// thread in a worktree; it is not an action of a client.
    ///
    /// # Errors
    /// As [`worktree::make`].
    pub async fn worktree(&self, plan: worktree::Plan<'_>) -> Result<worktree::Made, GitRefusal> {
        // One worktree of a thread is made at a time, whatever its repository.
        let _alone = self.alone(plan.thread, "").await;
        worktree::make(&self.git, &self.home, plan).await
    }

    /// Takes the checkpoint of a turn that ended. The runner calls it at each end of a turn.
    ///
    /// # Errors
    /// A [`GitRefusal`] when the thread has no worktree or `git` fails.
    pub async fn turn_ended(&self, target: Target<'_>, turn: u32) -> Result<(), GitRefusal> {
        let _alone = self.alone(target.thread, target.repository).await;
        let _index = self.index(target).await;
        let worktree = self.home.worktree(target.thread, target.repository)?;
        let git = self.git.for_worktree(&worktree).await?;
        checkpoint::capture(&git, &worktree, turn).await?;
        Ok(())
    }

    /// Does `action` for `caller` and gives the answer for the socket.
    pub async fn call(&self, caller: Caller, target: Target<'_>, action: GitAction) -> GitOutcome {
        if caller.agent && action.for_the_person() {
            return GitOutcome::Refused {
                why: GitRefusal::FromAnAgent,
            };
        }
        // An agent, and each caller that the runner bound to a thread, reads that thread alone.
        let own_thread = match caller.thread {
            Some(thread) => thread == target.thread,
            None => !caller.agent,
        };
        // An action that changes something waits for the one before it on this worktree.
        let _alone = if action.for_the_person() {
            Some(self.alone(target.thread, target.repository).await)
        } else {
            None
        };
        // Each use of the index of the runner waits for the one before it; a push lets go of
        // it before it speaks to the remote, so that the page can read while a push runs.
        let index = self.index(target).await;
        let done = match self.home.worktree(target.thread, target.repository) {
            Ok(worktree) if own_thread => match action {
                GitAction::Push {
                    default_branch_allowed,
                } => {
                    let asked = (caller, default_branch_allowed);
                    self.push(asked, &worktree, index).await
                }
                action => self.act(caller, &worktree, action).await,
            },
            Ok(_) => Err(GitRefusal::NoWorktree),
            Err(bad) => Err(bad.into()),
        };
        done.unwrap_or_else(|why| GitOutcome::Refused { why })
    }

    /// The lock of the index of the runner for the worktree of `target`.
    async fn index(&self, target: Target<'_>) -> tokio::sync::OwnedMutexGuard<()> {
        let name = format!("{}/index", target.repository);
        self.alone(target.thread, &name).await
    }

    /// A push: the checks with the index held, then the remote without it.
    async fn push(
        &self,
        (caller, default_branch_allowed): (Caller, bool),
        worktree: &Worktree,
        index: tokio::sync::OwnedMutexGuard<()>,
    ) -> Result<GitOutcome, GitRefusal> {
        if caller.running {
            return Err(GitRefusal::Running);
        }
        let git = &self.git.for_worktree(worktree).await?;
        let listed = &project::settings(git, &worktree.layout(), worktree.folder()).await?;
        trust::accepted(worktree, listed)?;
        let ready = push::ready(git, worktree, listed, default_branch_allowed).await?;
        drop(index);
        push::send(git, worktree, ready).await
    }

    async fn act(
        &self,
        caller: Caller,
        worktree: &Worktree,
        action: GitAction,
    ) -> Result<GitOutcome, GitRefusal> {
        let git = &self.git.for_worktree(worktree).await?;
        // While a turn runs the files change: nothing is reverted, removed or committed.
        if caller.running && waits_for_the_turn(&action) {
            return Err(GitRefusal::Running);
        }
        let layout = &worktree.layout();
        let listed = || project::settings(git, layout, worktree.folder());
        Ok(match action {
            GitAction::Status => GitOutcome::Status(shown(git, worktree, &listed().await?).await?),
            GitAction::Checkpoints => GitOutcome::Checkpoints {
                checkpoints: checkpoint::list(git, worktree).await?,
            },
            GitAction::Changes {
                scope,
                ignore_whitespace,
            } => {
                let shown = (scope, ignore_whitespace);
                GitOutcome::Changes(shown_changes(git, worktree, shown, !caller.agent).await?)
            }
            GitAction::Revert { turn } => GitOutcome::Reverted {
                turn,
                undo: revert::to_turn(git, worktree, turn).await?,
            },
            GitAction::Commit {
                message,
                reviewed,
                files,
                new_branch,
            } => {
                let listed = listed().await?;
                trust::accepted(worktree, &listed)?;
                let asked = commit::Asked {
                    message: &message,
                    reviewed: &reviewed,
                    files: files.as_deref(),
                    new_branch: new_branch.as_deref(),
                    who: &run::settings::person(&listed),
                };
                // A branch that this call makes is written to the record as one of the thread.
                let own = |name: &str| self.rewrite(worktree, |r| r.branches.push(name.to_owned()));
                let (commit, branch) = commit::make(git, worktree, asked, &own).await?;
                GitOutcome::Committed { commit, branch }
            }
            GitAction::PullRequest => GitOutcome::PullRequest(
                pull_request::prepare(git, worktree, &listed().await?).await?,
            ),
            GitAction::Remove => GitOutcome::Removed {
                kept: Some(worktree::remove(&self.git, &self.home, worktree).await?),
            },
            GitAction::Trust { shown } => self.trust(worktree, &listed().await?, &shown)?,
            // A push has its own way through the locks ([`Engine::push`]).
            GitAction::Push { .. } => return Err(GitRefusal::Running),
        })
    }

    /// Writes down that the person accepts who a commit is from and where a push goes, as
    /// they are now, when they are what the page showed (`shown`) and each could be shown.
    fn trust(
        &self,
        worktree: &Worktree,
        listed: &[Setting],
        shown: &str,
    ) -> Result<GitOutcome, GitRefusal> {
        let seen = trust::now(listed);
        let what = trust::changed(worktree.record().trusted.as_ref(), &seen);
        if what.len() > trust::MOST_NAMES {
            return Err(GitRefusal::Failed {
                said: "too many settings changed to show them; nothing is accepted".to_owned(),
            });
        }
        if trust::digest(&seen) != shown {
            return Err(GitRefusal::Changed {
                what: trust::first(what),
            });
        }
        trust::keep_baseline(&self.home, &worktree.record().project, &seen)?;
        self.rewrite(worktree, |record| record.trusted = Some(seen))?;
        Ok(GitOutcome::Trusted)
    }

    /// Changes the record of the worktree and writes it: a branch that the runner made for the
    /// thread, or what the person accepted.
    fn rewrite(
        &self,
        worktree: &Worktree,
        change: impl FnOnce(&mut Record),
    ) -> Result<(), GitRefusal> {
        let mut record = worktree.record().clone();
        change(&mut record);
        let repository = record.folder.file_name().and_then(|name| name.to_str());
        let repository = repository.ok_or(GitRefusal::BadPath)?;
        Ok(self.home.keep(worktree.thread(), repository, &record)?)
    }
}

/// The changes for the page. With `keep`, the snapshot of the uncommitted files goes under a
/// ref: a commit of the person takes only the one that they last looked at.
async fn shown_changes(
    git: &Git,
    worktree: &Worktree,
    (scope, ignore_whitespace): (Scope, bool),
    keep: bool,
) -> Result<Changes, GitRefusal> {
    let changes = changes::of(git, worktree, scope, ignore_whitespace).await?;
    if let (true, Some(shown)) = (keep, &changes.snapshot) {
        let name = checkpoint::reviewed_ref(worktree.thread());
        checkpoint::keep(git, worktree, &name, shown).await?;
    }
    Ok(changes)
}

/// The action is refused while a turn of the thread runs.
const fn waits_for_the_turn(action: &GitAction) -> bool {
    matches!(
        action,
        GitAction::Revert { .. }
            | GitAction::Remove
            | GitAction::Commit { .. }
            | GitAction::Push { .. }
            | GitAction::Trust { .. }
    )
}

/// The status with what changed since the person accepted it.
async fn shown(
    git: &Git,
    worktree: &Worktree,
    listed: &[Setting],
) -> Result<agent_threads::git::Status, GitRefusal> {
    let mut status = status::of(git, worktree, listed).await?;
    let seen = trust::now(listed);
    status.changed = trust::first(trust::changed(worktree.record().trusted.as_ref(), &seen));
    status.accept = trust::digest(&seen);
    Ok(status)
}
