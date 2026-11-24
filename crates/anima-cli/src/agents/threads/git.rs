//! Local worktree and checkpoint commands (TC7 W2). The runner owns Git and its records.

mod render;

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::git::{GitAction, GitOutcome, GitRefusal, Scope};
use uuid::Uuid;

use crate::agents::runner::{self, RunnerLink};
use crate::failure::{Failure, Hint, exit};
use crate::invocation::Invocation;
use crate::output::text::clean;

#[derive(Debug, clap::Args)]
pub struct GitArgs {
    /// ID from `anima agents running`.
    #[arg(long)]
    thread: Uuid,
    /// Name of the repository as it appears in the thread's worktree path.
    #[arg(long)]
    repository: String,
    #[command(subcommand)]
    command: GitCommand,
}

#[derive(Debug, clap::Subcommand)]
enum GitCommand {
    /// Show branch, upstream, changed files and settings that need acceptance.
    Status,
    /// List checkpoints, oldest first.
    Checkpoints,
    /// Show patches and counts. The default is the uncommitted changes.
    Changes(ChangesArgs),
    /// Restore the files at a checkpoint.
    Revert { turn: u32 },
    /// Commit the snapshot shown by `changes`. Without --file, commit every file in it.
    Commit {
        #[arg(long, short)]
        message: String,
        /// Snapshot from `changes` for this worktree.
        #[arg(long)]
        reviewed: String,
        /// Commit this path. Repeat to select more paths.
        #[arg(long = "file")]
        files: Option<Vec<String>>,
        /// Commit on a new branch with this name.
        #[arg(long)]
        new_branch: Option<String>,
    },
    /// Push the branch without force.
    Push {
        /// Allow pushing the default branch. Use only after checking the branch and remote.
        #[arg(long)]
        allow_default_branch: bool,
    },
    /// Print the address of the pull request page for a pushed branch.
    #[command(visible_alias = "pull-request")]
    Pr,
    /// Save the worktree under a private ref and remove it. Refuses if files would be lost.
    Remove,
    /// Discard checkpoints strictly before this turn.
    Forget { before: u32 },
    /// Accept only the Git settings identified by the digest shown in `status`.
    Trust {
        /// The acceptance digest from the status you reviewed.
        #[arg(long)]
        shown: String,
    },
}

#[derive(Debug, clap::Args)]
struct ChangesArgs {
    /// Compare this turn's checkpoint with the one before it.
    #[arg(long, conflicts_with = "whole_thread")]
    turn: Option<u32>,
    /// Compare the first and last checkpoints of the thread.
    #[arg(long)]
    whole_thread: bool,
    /// Ignore whitespace in the comparison.
    #[arg(long)]
    ignore_whitespace: bool,
}

impl GitArgs {
    pub(crate) async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        self.command
            .confirm(invocation, &self.repository, self.thread)?;
        let mut link = RunnerLink::connect().await?;
        let pull_request = matches!(self.command, GitCommand::Pr);
        let call = Action::Git {
            thread: self.thread,
            repository: self.repository,
            action: self.command.into(),
        };
        match link.call(call).await? {
            Outcome::Git {
                result: GitOutcome::Refused { why },
            }
            | Outcome::Refused {
                why: Refusal::Worktree { why },
            } => {
                if pull_request && why == GitRefusal::DefaultBranch {
                    return Err(Failure::Forbidden {
                        message: "The current branch is the default branch.".to_owned(),
                        hint: Some(Hint::say(
                            "Push a feature branch before asking for its pull request page.",
                        )),
                    });
                }
                Err(refused(why))
            }
            Outcome::Git { result } => render::show(&result, invocation.output),
            Outcome::Refused {
                why: Refusal::FromAnAgent,
            } => Err(refused(GitRefusal::FromAnAgent)),
            Outcome::Refused { why } => Err(runner::refusal(&why)),
            _ => Err(Failure::Input {
                what: "the Git reply from the thread runner".to_owned(),
                reason: "expected a Git result".to_owned(),
            }),
        }
    }
}

impl GitCommand {
    fn confirm(
        &self,
        invocation: &Invocation,
        repository: &str,
        thread: Uuid,
    ) -> Result<(), Failure> {
        let question = match self {
            Self::Revert { turn } => {
                format!("Restore {repository} in thread {thread} to turn {turn}?")
            }
            Self::Remove => {
                format!(
                    "Save the {repository} worktree of thread {thread} to Git and remove it if the files can be preserved?"
                )
            }
            Self::Forget { before } => {
                format!(
                    "Discard the checkpoints of {repository} in thread {thread} before turn {before}?"
                )
            }
            _ => return Ok(()),
        };
        invocation.prompter.confirm(&question)
    }
}

impl From<GitCommand> for GitAction {
    fn from(command: GitCommand) -> Self {
        match command {
            GitCommand::Status => Self::Status,
            GitCommand::Checkpoints => Self::Checkpoints,
            GitCommand::Changes(args) => Self::Changes {
                scope: args.scope(),
                ignore_whitespace: args.ignore_whitespace,
            },
            GitCommand::Revert { turn } => Self::Revert { turn },
            GitCommand::Commit {
                message,
                reviewed,
                files,
                new_branch,
            } => Self::Commit {
                message,
                reviewed,
                files,
                new_branch,
            },
            GitCommand::Push {
                allow_default_branch,
            } => Self::Push {
                default_branch_allowed: allow_default_branch,
            },
            GitCommand::Pr => Self::PullRequest,
            GitCommand::Remove => Self::Remove,
            GitCommand::Forget { before } => Self::Forget { before },
            GitCommand::Trust { shown } => Self::Trust { shown },
        }
    }
}

impl ChangesArgs {
    fn scope(&self) -> Scope {
        match self.turn {
            Some(turn) => Scope::Turn { turn },
            None if self.whole_thread => Scope::Thread,
            None => Scope::Uncommitted,
        }
    }
}

fn refused(why: GitRefusal) -> Failure {
    let message = match why {
        GitRefusal::FromAnAgent => "Only the person can ask for this Git action.",
        GitRefusal::NoWorktree => "This thread has no worktree for that repository.",
        GitRefusal::BadPath => "A path or name is not allowed.",
        GitRefusal::Running => "Interrupt the current turn before trying this Git action again.",
        GitRefusal::UnknownCheckpoint => "That checkpoint is unavailable.",
        GitRefusal::NothingToCommit => "There are no changes to commit.",
        GitRefusal::EmptyMessage => "Give a commit message.",
        GitRefusal::DetachedHead => "Check out a branch first. HEAD is detached.",
        GitRefusal::NoRemote => "This repository has no push remote.",
        GitRefusal::Behind => "The branch is behind its upstream. Pull or rebase before pushing.",
        GitRefusal::DefaultBranch => {
            return Failure::Forbidden {
                message: "This is the default branch.".to_owned(),
                hint: Some(Hint::say(
                    "Check the branch and remote, then pass --allow-default-branch to push it.",
                )),
            };
        }
        GitRefusal::NotOurBranch => "This branch belongs to another checkout. Use --new-branch.",
        GitRefusal::NotPushed => "Push the branch before asking for its pull request page.",
        GitRefusal::NoCommits => "There are no commits to include in a pull request.",
        GitRefusal::UnknownForge => "The remote is not on a forge supported by this command.",
        GitRefusal::BranchExists => "A branch with that name already exists.",
        GitRefusal::UnknownBase => "That base branch or commit is unavailable.",
        GitRefusal::NotARepository => "That folder is not the top folder of a Git repository.",
        GitRefusal::GitNotFound => "Git is not on the PATH of your login.",
        GitRefusal::Changed { what } => {
            let names = what.iter().map(|name| clean(name)).collect::<Vec<_>>();
            return Failure::Conflict {
                message: format!(
                    "Git settings changed. Read status and accept the new digest before retrying. Settings: {}",
                    names.join(", ")
                ),
            };
        }
        GitRefusal::NotReviewed => {
            "Read the complete uncommitted changes without --ignore-whitespace and use the new snapshot."
        }
        GitRefusal::RecordsHeld => {
            "A running thread can write to Anima's Git records. Stop that thread first."
        }
        GitRefusal::Expired => "The checkpoints expired. The worktree is still on disk.",
        GitRefusal::NotOnWindows => "Thread worktrees and checkpoints are unavailable on Windows.",
        GitRefusal::Failed { said } => {
            return Failure::Ended {
                message: said,
                status: exit::FAILURE,
            };
        }
    };
    Failure::Forbidden {
        message: message.to_owned(),
        hint: None,
    }
}
