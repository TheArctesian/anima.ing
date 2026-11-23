//! `anima record-link`: the links between records of different extensions (RLK-001, RLK-002,
//! RLK-004).

pub mod change;
pub mod documents;
pub mod list;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum RecordLinkCommand {
    /// List the links that start or end at a record.
    List(list::ListArgs),
    /// Link one record to another.
    Add(change::AddArgs),
    /// Remove a link; the two records stay.
    Remove(change::RemoveArgs),
}

impl RecordLinkCommand {
    /// # Errors
    /// [`Failure`] for a refusal (`VALIDATION`, `NOT_FOUND`) or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::List(args) => args.run(invocation).await,
            Self::Add(args) => args.run(invocation).await,
            Self::Remove(args) => args.run(invocation).await,
        }
    }
}
