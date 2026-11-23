//! `anima event` and `anima audit`.

pub mod audit;
pub mod documents;
pub mod list;
pub mod verify;
pub mod watch;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum EventCommand {
    /// List events, newest first, with filters and pages.
    List(list::ListArgs),
    /// Print each new event until interrupted.
    Watch(watch::WatchArgs),
}

#[derive(Debug, clap::Subcommand)]
pub enum AuditCommand {
    /// List the audit trail, newest first.
    List(audit::ListArgs),
    /// Check that no audit entry was changed or removed (the hash chain).
    Verify(verify::VerifyArgs),
}

impl EventCommand {
    /// # Errors
    /// [`Failure`] for a refusal, an unknown device or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::List(args) => args.run(invocation).await,
            Self::Watch(args) => args.run(invocation).await,
        }
    }
}

impl AuditCommand {
    /// # Errors
    /// [`Failure`] for a refusal or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::List(args) => args.run(invocation).await,
            Self::Verify(args) => args.run(invocation).await,
        }
    }
}
