//! `rotate`, `revoke` and `remove`: each ends a version, and the answer names the servers whose
//! daemon still held it at its last report (K-E5 Section 2).

use std::path::PathBuf;

use serde_json::Value;
use zeroize::Zeroize as _;

use super::documents::{
    SecretRemove, SecretRevoke, SecretRotate, secret_remove, secret_revoke, secret_rotate,
};
use super::shown::{as_json, consumer_line, list_of, record};
use super::value;
use crate::devices::credentials::input::send_wiping;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::Table;

#[derive(clap::Args)]
pub struct RotateArgs {
    /// The name of the key.
    name: String,
    /// Say that the key was also changed at its provider. Without it the new value is recorded
    /// only: the hub changes nothing at the provider.
    #[arg(long)]
    rotated_at_provider: bool,
    /// Read the value from this file (mode 0600, yours), or from standard input for `-`.
    /// Without it, a terminal asks with no echo and a pipe is read.
    #[arg(long, value_name = "PATH")]
    from_file: Option<PathBuf>,
}

/// Debug output names the file, never the value it holds.
impl std::fmt::Debug for RotateArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RotateArgs")
            .field("name", &self.name)
            .field("rotated_at_provider", &self.rotated_at_provider)
            .field("from_file", &self.from_file)
            .finish()
    }
}

impl RotateArgs {
    /// Reads the new value and makes it the active version.
    ///
    /// # Errors
    /// [`Failure`] for an unusable value or the hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let value = value::read(invocation.prompter, self.from_file.as_deref())?;
        invocation.prompter.confirm(&format!(
            "Make the new value active for {}? The old version ends at once.",
            self.name
        ))?;
        let hub = invocation.hub()?;
        let variables = secret_rotate::Variables {
            name: self.name.clone(),
            value: value.as_str().to_owned(),
            rotated_at_provider: self.rotated_at_provider,
        };
        let answer = send_wiping::<SecretRotate>(&hub, variables, |variables| {
            variables.value.zeroize();
        })
        .await?;
        let verb = if self.rotated_at_provider {
            "rotated"
        } else {
            "recorded; the hub changed nothing at the provider"
        };
        invocation
            .output
            .note(&format!("The new value of {} is {verb}.", self.name));
        ended(invocation, &as_json(&answer.rotate_operational_key))
    }
}

/// # Errors
/// [`Failure`] for a declined confirmation or the hub's refusal.
pub async fn revoke(invocation: &Invocation, name: String) -> Result<(), Failure> {
    invocation.prompter.confirm(&format!(
        "Revoke the active version of {name}? Servers get no value until a rotation."
    ))?;
    let hub = invocation.hub()?;
    let answer = hub
        .run::<SecretRevoke>(secret_revoke::Variables { name })
        .await?;
    ended(invocation, &as_json(&answer.revoke_operational_key))
}

/// # Errors
/// [`Failure`] for a declined confirmation or the hub's refusal.
pub async fn remove(invocation: &Invocation, name: String) -> Result<(), Failure> {
    invocation.prompter.confirm(&format!(
        "Remove {name}? Servers get it no more, and the name stays taken."
    ))?;
    let hub = invocation.hub()?;
    let answer = hub
        .run::<SecretRemove>(secret_remove::Variables { name })
        .await?;
    ended(invocation, &as_json(&answer.remove_operational_key))
}

/// Prints the key, and says which servers keep a copy of the ended version.
fn ended(invocation: &Invocation, ending: &Value) -> Result<(), Failure> {
    let held = list_of(ending, "stillHeldBy");
    if !held.is_empty() {
        invocation.output.note(
            "These servers held the ended version at their last report. A connected daemon \
             drops it at its next report; a server with no contact keeps its copy, so revoke \
             the key at the provider when a server is lost.",
        );
    }
    invocation.output.data(ending, |ending| {
        let mut table: Table = record(&ending["key"]);
        table.pair(
            "Ended version",
            ending["endedVersion"]
                .as_i64()
                .map_or_else(|| "none".to_owned(), |version| version.to_string()),
        );
        for consumer in list_of(ending, "stillHeldBy") {
            table.pair("Still held by", consumer_line(consumer));
        }
        table
    })
}
