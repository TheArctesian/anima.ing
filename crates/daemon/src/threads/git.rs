//! The Git of a thread (stage 7 of the copy of T3 Code, tasks TH41 to TH45): a worktree for
//! each thread, a checkpoint at each end of a turn, the changes, the revert, and the commit, the
//! push and the pull request that the person asks for.
//!
//! [`Engine::call`] is the one door for a client. It refuses an agent each action that changes
//! something (THR-049), and it finds the worktree by the ID of the thread alone, so that no
//! path of a client reaches `git` and the checkout of the person is never changed (P12).
//!
//! Pure: [`place`] (the checks), the readers of [`changes`], [`status`] and [`pull_request`].
//! I/O: [`run`] starts `git`; the other modules call it.

pub mod changes;
pub mod checkpoint;
pub mod commit;
pub mod place;
pub mod pull_request;
pub mod push;
pub mod revert;
pub mod run;
pub mod status;
#[cfg(test)]
mod tests;
pub mod trust;
pub mod worktree;

use agent_threads::git::{Changes, GitAction, GitOutcome, GitRefusal, Scope};
use uuid::Uuid;

use place::{BadPlace, Home, Record, Worktree};
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

/// The Git of the threads of this computer.
#[derive(Clone, Debug)]
pub struct Engine {
    git: Git,
    home: Home,
}

impl Engine {
    #[must_use]
    pub const fn new(git: Git, home: Home) -> Self {
        Self { git, home }
    }

    /// Makes or finds the worktree of a thread. The runner calls it when the person starts a
    /// thread in a worktree; it is not an action of a client.
    ///
    /// # Errors
    /// As [`worktree::make`].
    pub async fn worktree(&self, plan: worktree::Plan<'_>) -> Result<worktree::Made, GitRefusal> {
        worktree::make(&self.git, &self.home, plan).await
    }

    /// Takes the checkpoint of a turn that ended. The runner calls it at each end of a turn.
    ///
    /// # Errors
    /// A [`GitRefusal`] when the thread has no worktree or `git` fails.
    pub async fn turn_ended(&self, target: Target<'_>, turn: u32) -> Result<(), GitRefusal> {
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
        let done = match self.home.worktree(target.thread, target.repository) {
            Ok(worktree) if own_thread => self.act(caller, &worktree, action).await,
            Ok(_) => Err(GitRefusal::NoWorktree),
            Err(bad) => Err(bad.into()),
        };
        done.unwrap_or_else(|why| GitOutcome::Refused { why })
    }

    async fn act(
        &self,
        caller: Caller,
        worktree: &Worktree,
        action: GitAction,
    ) -> Result<GitOutcome, GitRefusal> {
        let git = &self.git.for_worktree(worktree).await?;
        // While a turn runs the files change: nothing is reverted, removed, committed or pushed.
        if caller.running && waits_for_the_turn(&action) {
            return Err(GitRefusal::Running);
        }
        Ok(match action {
            GitAction::Status => GitOutcome::Status(shown(git, worktree).await?),
            GitAction::Checkpoints => GitOutcome::Checkpoints {
                checkpoints: checkpoint::list(git, worktree).await?,
            },
            GitAction::Changes {
                scope,
                ignore_whitespace,
            } => GitOutcome::Changes(changes_of(git, worktree, scope, ignore_whitespace).await?),
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
                accepted(git, worktree).await?;
                let asked = commit::Asked {
                    message: &message,
                    reviewed: &reviewed,
                    files: files.as_deref(),
                    new_branch: new_branch.as_deref(),
                };
                // A branch that this call makes is written to the record as one of the thread.
                let own = |name: &str| self.rewrite(worktree, |r| r.branches.push(name.to_owned()));
                let (commit, branch) = commit::make(git, worktree, asked, &own).await?;
                GitOutcome::Committed { commit, branch }
            }
            GitAction::Push {
                default_branch_allowed,
            } => {
                accepted(git, worktree).await?;
                push::make(git, worktree, default_branch_allowed).await?
            }
            GitAction::PullRequest => {
                GitOutcome::PullRequest(pull_request::prepare(git, worktree).await?)
            }
            GitAction::Remove => GitOutcome::Removed {
                kept: Some(worktree::remove(&self.git, &self.home, worktree).await?),
            },
            GitAction::Trust => {
                let seen = trust::now(git, worktree).await?;
                self.rewrite(worktree, |record| record.trusted = Some(seen.entries))?;
                GitOutcome::Trusted
            }
        })
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

/// The action is refused while a turn of the thread runs.
const fn waits_for_the_turn(action: &GitAction) -> bool {
    matches!(
        action,
        GitAction::Revert { .. }
            | GitAction::Remove
            | GitAction::Commit { .. }
            | GitAction::Push { .. }
    )
}

/// Refuses when a hook or a setting of the repository is not as the person accepted it: a
/// commit or a push would run or obey it (THR-049).
async fn accepted(git: &Git, worktree: &Worktree) -> Result<(), GitRefusal> {
    let seen = trust::now(git, worktree).await?;
    let what = trust::changed(worktree.record().trusted.as_ref(), &seen.entries);
    if what.is_empty() {
        Ok(())
    } else {
        Err(GitRefusal::Changed { what })
    }
}

/// The status with the hooks that a commit runs and what changed since the person accepted.
async fn shown(git: &Git, worktree: &Worktree) -> Result<agent_threads::git::Status, GitRefusal> {
    let mut status = status::of(git, worktree).await?;
    let seen = trust::now(git, worktree).await?;
    status.changed = trust::changed(worktree.record().trusted.as_ref(), &seen.entries);
    status.hooks = seen.hooks;
    Ok(status)
}

/// The changes of `scope`: between two checkpoints, or from the last commit to the files now.
async fn changes_of(
    git: &Git,
    worktree: &Worktree,
    scope: Scope,
    ignore_whitespace: bool,
) -> Result<Changes, GitRefusal> {
    {
        let turns = checkpoint::list(git, worktree).await?;
        let at = |turn: u32| {
            turns
                .iter()
                .find(|checkpoint| checkpoint.turn == turn)
                .map(|checkpoint| checkpoint.commit.clone())
                .ok_or(GitRefusal::UnknownCheckpoint)
        };
        let (from, to) = match scope {
            Scope::Uncommitted => (
                "HEAD".to_owned(),
                checkpoint::snapshot(git, worktree).await?,
            ),
            Scope::Turn { turn } => (
                at(turn.checked_sub(1).ok_or(GitRefusal::UnknownCheckpoint)?)?,
                at(turn)?,
            ),
            Scope::Thread => match (turns.first(), turns.last()) {
                (Some(first), Some(last)) => (first.commit.clone(), last.commit.clone()),
                _ => return Err(GitRefusal::UnknownCheckpoint),
            },
        };
        let mut changes = changes::between(git, worktree, (&from, &to), ignore_whitespace).await?;
        // A commit of the person names this snapshot: it holds what the page shows.
        changes.snapshot = (scope == Scope::Uncommitted).then_some(to);
        Ok(changes)
    }
}
