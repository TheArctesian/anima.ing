//! `anima secret`: the operational keys of the operator's servers (K-E5 Section 2.2, OD-E23).
//! A value goes in through a prompt with no echo, standard input or a 0600 file, and never comes
//! out; `anima key` is the noun of the API keys.

pub mod documents;
mod endings;
mod grants;
mod metadata;
mod shown;
mod value;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum SecretCommand {
    /// List the operational keys: provider, due state, active version and consumers. Never a value.
    List,
    /// Show one key with its versions and its consumers. Never a value.
    Show(NameArgs),
    /// Make a key with its first value, which comes from a prompt with no echo, standard input
    /// or --from-file.
    Add(metadata::AddArgs),
    /// Change the metadata of a key: only what you give changes.
    Set(metadata::SetArgs),
    /// Record a new value as the active version; the old version ends at once.
    Rotate(endings::RotateArgs),
    /// End the active version with no new one.
    Revoke(NameArgs),
    /// End the key and its retrieval; the name stays taken.
    Remove(NameArgs),
    /// Let the daemon of a server fetch the key for one workload.
    Grant(grants::GrantArgs),
    /// End a grant; the daemon drops the key at its next report.
    Ungrant(grants::GrantArgs),
}

#[derive(Debug, clap::Args)]
pub struct NameArgs {
    /// The name of the key.
    name: String,
}

impl SecretCommand {
    /// Runs the command.
    ///
    /// # Errors
    /// [`Failure`] for a refusal (`FORBIDDEN` for an API key on a change, `VALIDATION`,
    /// `NOT_FOUND`, `CONFLICT`), an unusable value, a declined confirmation or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::List => shown::list(invocation).await,
            Self::Show(args) => shown::show(invocation, args.name).await,
            Self::Add(args) => args.run(invocation).await,
            Self::Set(args) => args.run(invocation).await,
            Self::Rotate(args) => args.run(invocation).await,
            Self::Revoke(args) => endings::revoke(invocation, args.name).await,
            Self::Remove(args) => endings::remove(invocation, args.name).await,
            Self::Grant(args) => args.grant(invocation).await,
            Self::Ungrant(args) => args.ungrant(invocation).await,
        }
    }
}
