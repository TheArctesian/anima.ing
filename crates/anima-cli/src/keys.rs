//! `anima key`: API keys for scripts and coding agents; only a login token makes, lists or
//! revokes them (INT-003).

pub mod create;
pub mod documents;
pub mod expiry;
pub mod list;
pub mod revoke;

use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Subcommand)]
pub enum KeyCommand {
    /// List API keys: name, scope, prefix and expiry. Never the secret.
    List(list::ListArgs),
    /// Make an API key and print its secret, once.
    Create(create::CreateArgs),
    /// Revoke an API key by ID or exact name.
    Revoke(revoke::RevokeArgs),
}

impl KeyCommand {
    /// # Errors
    /// [`Failure`] for a refusal, an unknown key or a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::List(args) => args.run(invocation).await,
            Self::Create(args) => args.run(invocation).await,
            Self::Revoke(args) => args.run(invocation).await,
        }
    }
}
