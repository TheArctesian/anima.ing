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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use place::{BadPlace, Home, Record, Worktree};
use push::pull_request;
use run::hooks::{self, Access, AsThread};
use run::{Git, GitError};

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
            BadPlace::Expired => Self::Expired,
            BadPlace::Escapes | BadPlace::BadName | BadPlace::InARepository | BadPlace::KeptOut => {
                Self::BadPath
            }
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
    /// How much the agent of the thread may do: only a thread with full access runs the
    /// hooks of the repository ([`run::hooks::of`]).
    pub access: Access,
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
    /// How a hook of the repository is started with the rights of a thread; `None` until the
    /// runner gives it, and then no hook runs for any thread.
    as_thread: Option<AsThread>,
    /// A thread runs in a folder that holds the records ([`Engine::hold`]): nothing runs.
    held: Arc<AtomicBool>,
}

impl Engine {
    #[must_use]
    pub fn new(git: Git, home: Home) -> Self {
        Self {
            git,
            home,
            locks: Arc::default(),
            as_thread: None,
            held: Arc::default(),
        }
    }

    /// The runner says whether a thread of this computer runs in a folder that holds the
    /// records ([`place::records::held_by`]): its agent can write them, and with them the
    /// Git folder that the runner's `git` runs with. While it is so, each Git action of each
    /// thread is refused (THR-050). Each clone of the engine sees the same answer.
    pub fn hold(&self, held: bool) {
        self.held.store(held, Ordering::SeqCst);
    }

    /// Refuses while the records can be written by a thread, or are not a folder of this
    /// user alone any more.
    fn closed(&self) -> Result<(), GitRefusal> {
        if self.held.load(Ordering::SeqCst) {
            return Err(GitRefusal::RecordsHeld);
        }
        place::records::checked(self.home.records()).map_err(|bad| GitRefusal::Failed {
            said: bad.to_string(),
        })
    }

    /// This engine with the way to start a hook of a repository as the agent of a thread is
    /// started. Without it no hook of a repository runs.
    #[must_use]
    pub fn with_launcher(mut self, as_thread: AsThread) -> Self {
        self.git = self.git.with_thread(as_thread.clone());
        self.as_thread = Some(as_thread);
        self
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
        self.worktree_shown(plan, None).await
    }

    /// As [`Engine::worktree`]; `shown` hears each step, for the page. Dropping the call
    /// cancels it: what it made so far goes again.
    ///
    /// # Errors
    /// As [`worktree::make`].
    pub async fn worktree_shown(
        &self,
        plan: worktree::Plan<'_>,
        shown: worktree::Shown<'_>,
    ) -> Result<worktree::Made, GitRefusal> {
        self.closed()?;
        // One worktree of a thread is made at a time, whatever its repository.
        let _alone = self.alone(plan.thread, "").await;
        // One worktree of a project at a time: the first one copies its history, and the
        // next one finds the copy.
        let real = std::fs::canonicalize(plan.project);
        let project = real.as_deref().unwrap_or(plan.project).to_string_lossy();
        let _copy = self.alone(Uuid::nil(), &project).await;
        worktree::make(&self.git, &self.home, plan, shown).await
    }

    /// Takes the checkpoint of a turn that ended. The runner calls it at each end of a turn.
    ///
    /// # Errors
    /// A [`GitRefusal`] when the thread has no worktree or `git` fails.
    pub async fn turn_ended(&self, target: Target<'_>, turn: u32) -> Result<(), GitRefusal> {
        self.closed()?;
        let _alone = self.alone(target.thread, target.repository).await;
        let _index = self.index(target).await;
        let worktree = self.home.worktree(target.thread, target.repository)?;
        let git = self.git.for_worktree(&worktree).await?;
        checkpoint::capture(&git, &worktree, turn).await?;
        Ok(())
    }

    /// Does `action` for `caller` and gives the answer for the socket.
    pub async fn call(&self, caller: Caller, target: Target<'_>, action: GitAction) -> GitOutcome {
        if let Err(why) = self.closed() {
            return GitOutcome::Refused { why };
        }
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
        let hooks = hooks::of(caller.access, self.as_thread.as_ref(), (worktree, listed));
        trust::accepted(worktree, listed, hooks.as_ref())?;
        let ready = push::ready(git, worktree, listed, default_branch_allowed).await?;
        drop(index);
        push::send(git, worktree, ready, hooks.as_ref()).await
    }

    async fn act(
        &self,
        caller: Caller,
        worktree: &Worktree,
        action: GitAction,
    ) -> Result<GitOutcome, GitRefusal> {
        let git = &self.git.for_worktree(worktree).await?;
        // While a turn runs the files change: nothing is reverted, removed or committed.
        if caller.running && action.waits_for_the_turn() {
            return Err(GitRefusal::Running);
        }
        let layout = &worktree.layout();
        let listed = || project::settings(git, layout, worktree.folder());
        Ok(match action {
            GitAction::Status => self.accepting(caller, (git, worktree), None).await?,
            GitAction::Checkpoints => GitOutcome::Checkpoints {
                checkpoints: checkpoint::list(git, worktree).await?,
            },
            GitAction::Changes {
                scope,
                ignore_whitespace,
            } => {
                let shown = (scope, ignore_whitespace);
                GitOutcome::Changes(changes::shown(git, worktree, shown, !caller.agent).await?)
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
                let hooks = hooks::of(caller.access, self.as_thread.as_ref(), (worktree, &listed));
                trust::accepted(worktree, &listed, hooks.as_ref())?;
                let asked = commit::Asked {
                    message: &message,
                    reviewed: &reviewed,
                    files: files.as_deref(),
                    new_branch: new_branch.as_deref(),
                    who: &run::settings::person(&listed),
                    hooks: hooks.as_ref(),
                };
                // A branch that this call makes is written to the record as one of the thread.
                let own = |name: &str| self.rewrite(worktree, |r| r.branches.push(name.to_owned()));
                let (commit, branch) = commit::make(git, worktree, asked, &own).await?;
                GitOutcome::Committed { commit, branch }
            }
            GitAction::PullRequest => GitOutcome::PullRequest(
                pull_request::prepare(git, worktree, &listed().await?).await?,
            ),
            GitAction::Forget { before } => GitOutcome::Checkpoints {
                checkpoints: checkpoint::forget_before(git, worktree, before).await?,
            },
            GitAction::Remove => GitOutcome::Removed {
                kept: Some(worktree::remove(&self.git, &self.home, worktree).await?),
            },
            GitAction::Trust { shown } => {
                self.accepting(caller, (git, worktree), Some(&shown))
                    .await?
            }
            // A push has its own way through the locks ([`Engine::push`]).
            GitAction::Push { .. } => return Err(GitRefusal::Running),
        })
    }

    /// The status with what the person has to accept, or, with `shown`, their yes to it.
    async fn accepting(
        &self,
        caller: Caller,
        (git, worktree): (&Git, &Worktree),
        shown: Option<&str>,
    ) -> Result<GitOutcome, GitRefusal> {
        let listed = &project::settings(git, &worktree.layout(), worktree.folder()).await?;
        let hooks = hooks::of(caller.access, self.as_thread.as_ref(), (worktree, listed));
        if let Some(shown) = shown {
            let seen = trust::seen(worktree, listed, hooks.as_ref());
            return self.trust(worktree, seen, shown);
        }
        let status = trust::status_with(git, worktree, listed, hooks.as_ref());
        Ok(GitOutcome::Status(status.await?))
    }

    /// Writes down that the person accepts who a commit is from and where a push goes, as
    /// they are now, when they are what the page showed (`shown`) and each could be shown.
    fn trust(
        &self,
        worktree: &Worktree,
        seen: std::collections::BTreeMap<String, String>,
        shown: &str,
    ) -> Result<GitOutcome, GitRefusal> {
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
