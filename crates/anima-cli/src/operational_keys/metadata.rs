//! `add` and `set`: the metadata of a key, and the first value of a new one.

use std::path::PathBuf;

use zeroize::Zeroize as _;

use super::documents::{SecretAdd, SecretSet, SecretShow, secret_add, secret_set, secret_show};
use super::shown::{as_json, not_found, record};
use super::value;
use crate::devices::credentials::input::send_wiping;
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(clap::Args)]
pub struct AddArgs {
    /// The name: 1 to 80 of a-z, 0-9, _, . and -, starting with a letter.
    name: String,
    /// Who issued the key, such as cloudflare.
    #[arg(long)]
    provider: String,
    /// What the key is for.
    #[arg(long)]
    purpose: String,
    /// How to rotate the key at its provider.
    #[arg(long, default_value = "")]
    instructions: String,
    /// An https address at the provider.
    #[arg(long, value_name = "URL")]
    provider_link: Option<String>,
    /// The key is due for a rotation this many days after the last one.
    #[arg(long, value_name = "DAYS")]
    rotation_days: Option<i64>,
    /// When the key stops working at its provider (RFC 3339).
    #[arg(long, value_name = "TIME")]
    expires_at: Option<String>,
    /// Read the value from this file (mode 0600, yours), or from standard input for `-`.
    /// Without it, a terminal asks with no echo and a pipe is read. The value is never an
    /// argument.
    #[arg(long, value_name = "PATH")]
    from_file: Option<PathBuf>,
}

/// Debug output names the file, never the value it holds.
impl std::fmt::Debug for AddArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AddArgs")
            .field("name", &self.name)
            .field("from_file", &self.from_file)
            .finish_non_exhaustive()
    }
}

impl AddArgs {
    /// Reads the value, makes the key and prints its metadata.
    ///
    /// # Errors
    /// [`Failure`] for an unusable value or the hub's refusal (`CONFLICT` for a name that is
    /// taken, `FORBIDDEN` for an API key).
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let value = value::read(invocation.prompter, self.from_file.as_deref())?;
        let hub = invocation.hub()?;
        let variables = secret_add::Variables {
            input: secret_add::CreateOperationalKeyInput {
                name: self.name,
                provider: self.provider,
                purpose: self.purpose,
                instructions: self.instructions,
                provider_link: self.provider_link,
                rotation_days: self.rotation_days,
                expires_at: self.expires_at,
                value: value.as_str().to_owned(),
            },
        };
        let answer = send_wiping::<SecretAdd>(&hub, variables, |variables| {
            variables.input.value.zeroize();
        })
        .await?;
        let key = as_json(&answer.create_operational_key);
        invocation.output.data(&key, record)
    }
}

#[derive(Debug, clap::Args)]
pub struct SetArgs {
    /// The name of the key.
    name: String,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    purpose: Option<String>,
    #[arg(long)]
    instructions: Option<String>,
    /// An https address at the provider.
    #[arg(long, value_name = "URL", conflicts_with = "no_provider_link")]
    provider_link: Option<String>,
    /// Remove the provider link.
    #[arg(long)]
    no_provider_link: bool,
    #[arg(long, value_name = "DAYS", conflicts_with = "no_rotation_days")]
    rotation_days: Option<i64>,
    /// The key is never due for a rotation.
    #[arg(long)]
    no_rotation_days: bool,
    /// When the key stops working at its provider (RFC 3339).
    #[arg(long, value_name = "TIME", conflicts_with = "no_expiry")]
    expires_at: Option<String>,
    /// The key has no expiry.
    #[arg(long)]
    no_expiry: bool,
}

impl SetArgs {
    /// Reads the key, changes only the given fields and prints the result.
    ///
    /// # Errors
    /// [`Failure::NotFound`] for an unknown name; the hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let shown = hub
            .run::<SecretShow>(secret_show::Variables {
                name: self.name.clone(),
            })
            .await?;
        let now = shown.operational_key.ok_or_else(|| not_found(&self.name))?;
        let input = secret_set::UpdateOperationalKeyInput {
            provider: self.provider.unwrap_or(now.provider),
            purpose: self.purpose.unwrap_or(now.purpose),
            instructions: self.instructions.unwrap_or(now.instructions),
            provider_link: kept(self.provider_link, self.no_provider_link, now.provider_link),
            rotation_days: kept(self.rotation_days, self.no_rotation_days, now.rotation_days),
            expires_at: kept(self.expires_at, self.no_expiry, now.expires_at),
        };
        let variables = secret_set::Variables {
            name: self.name,
            input,
        };
        let answer = hub.run::<SecretSet>(variables).await?;
        let key = as_json(&answer.update_operational_key);
        invocation.output.data(&key, record)
    }
}

/// The new value, none when cleared, or the stored one.
fn kept<T>(given: Option<T>, cleared: bool, stored: Option<T>) -> Option<T> {
    if cleared { None } else { given.or(stored) }
}

#[cfg(test)]
mod tests {
    use super::kept;

    #[test]
    fn set_changes_only_what_is_given_and_clears_on_request() {
        assert_eq!(kept(None, false, Some(30)), Some(30));
        assert_eq!(kept(Some(7), false, Some(30)), Some(7));
        assert_eq!(kept(None, true, Some(30)), None);
    }
}
