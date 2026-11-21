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
pub mod said;
pub mod status;
#[cfg(test)]
mod tests;
pub mod worktree;

use agent_threads::git::{Changes, GitAction, GitOutcome, GitRefusal, Scope};
use uuid::Uuid;

use place::{BadPlace, Home, Worktree};
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
        let git = self.git.quiet_in(worktree.folder()).await?;
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
        let done = match self.home.worktree(target.thread, target.repository) {
            Ok(worktree) => self.act(caller, &worktree, action).await,
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
        let git = &self.git.quiet_in(worktree.folder()).await?;
        Ok(match action {
            GitAction::Status => GitOutcome::Status(status::of(git, worktree).await?),
            GitAction::Checkpoints => GitOutcome::Checkpoints {
                checkpoints: checkpoint::list(git, worktree).await?,
            },
            GitAction::Changes {
                scope,
                ignore_whitespace,
            } => GitOutcome::Changes(changes_of(git, worktree, scope, ignore_whitespace).await?),
            GitAction::Revert { .. } | GitAction::Remove if caller.running => {
                return Err(GitRefusal::Running);
            }
            GitAction::Revert { turn } => GitOutcome::Reverted {
                turn,
                undo: revert::to_turn(git, worktree, turn).await?,
            },
            GitAction::Commit {
                message,
                files,
                new_branch,
            } => {
                let asked = commit::Asked {
                    message: &message,
                    files: files.as_deref(),
                    new_branch: new_branch.as_deref(),
                };
                // A commit on the default branch moves a branch that the thread did not make.
                if asked.new_branch.is_none() && status::of(git, worktree).await?.is_default_branch
                {
                    return Err(GitRefusal::DefaultBranch);
                }
                let (commit, branch) = commit::make(git, worktree, asked).await?;
                GitOutcome::Committed { commit, branch }
            }
            GitAction::Push {
                default_branch_allowed,
            } => {
                let (remote, branch, up_to_date) =
                    push::make(git, worktree, default_branch_allowed).await?;
                GitOutcome::Pushed {
                    remote,
                    branch,
                    up_to_date,
                }
            }
            GitAction::PullRequest => {
                GitOutcome::PullRequest(pull_request::prepare(git, worktree).await?)
            }
            GitAction::Remove => GitOutcome::Removed {
                kept: Some(worktree::remove(git, &self.home, worktree).await?),
            },
        })
    }
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
        Ok(changes::between(git, worktree, (&from, &to), ignore_whitespace).await?)
    }
}
