//! GitHub repository nouns (PRJ-030 to PRJ-035): the hub reads GitHub; the command line only
//! asks and prints.

pub mod body;
pub mod comments;
pub mod render;
pub mod rows;

use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::{code, issues, pull_requests, repositories, workflows};

/// The nouns that read a repository on GitHub through the hub.
#[derive(Debug, clap::Subcommand)]
pub enum ForgeCommand {
    /// Pull requests of a repository (list, get, merge).
    Pr {
        #[command(subcommand)]
        command: pull_requests::PullRequestCommand,
    },
    /// Issues of a repository (list, get, create, comment, close, reopen).
    Issue {
        #[command(subcommand)]
        command: issues::IssueCommand,
    },
    /// Workflows of a repository and their runs (list, start, runs, run, rerun, cancel).
    Workflow {
        #[command(subcommand)]
        command: workflows::WorkflowCommand,
    },
    /// Browse the files of a repository through the hub.
    Forge {
        #[command(subcommand)]
        command: BrowseCommand,
    },
    /// The history of a repository on GitHub (commits, branches, releases, status set).
    Repo {
        #[command(subcommand)]
        command: repositories::RepositoryCommand,
    },
}

impl ForgeCommand {
    /// Runs the command.
    ///
    /// # Errors
    /// [`Failure`] for an unknown repository, pull request, issue or run, or a hub refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Pr { command } => command.run(invocation).await,
            Self::Issue { command } => command.run(invocation).await,
            Self::Workflow { command } => command.run(invocation).await,
            Self::Forge {
                command: BrowseCommand::Code(command),
            } => command.run(invocation).await,
            Self::Repo { command } => command.run(invocation).await,
        }
    }
}

/// `--state`: which pull requests or issues to list.
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum StateArg {
    #[default]
    Open,
    Closed,
    All,
}

impl StateArg {
    /// The word as `--state` takes it, for the next-page hint.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

/// Repository browsing stays distinct from agent-session commands under `anima code`.
#[derive(Debug, clap::Subcommand)]
pub enum BrowseCommand {
    /// List or read repository files.
    #[command(subcommand)]
    Code(code::CodeCommand),
}
