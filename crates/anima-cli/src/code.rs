//! `anima code`: repository files (PRJ-039, PRJ-040) and local thread worktrees (TC7 W2).

pub mod documents;
pub mod ls;
pub mod render;
pub mod show;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum CodeCommand {
    /// Read or change the Git worktree of a thread on this computer.
    #[cfg(unix)]
    Git(crate::agents::threads::git::GitArgs),
    /// List one directory of a repository: kind, size, name.
    Ls(ls::LsArgs),
    /// Print a text file exactly as it is (needs a login, not an API key).
    #[command(visible_alias = "cat")]
    Show(show::ShowArgs),
}

impl CodeCommand {
    /// Runs the command.
    ///
    /// # Errors
    /// [`Failure`] for an unknown repository, a path that is not a directory (or not a file),
    /// a file that cannot be shown, or a hub refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            #[cfg(unix)]
            Self::Git(args) => args.run(invocation).await,
            Self::Ls(args) => args.run(invocation).await,
            Self::Show(args) => args.run(invocation).await,
        }
    }
}
