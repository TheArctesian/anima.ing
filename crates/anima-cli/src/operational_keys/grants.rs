//! `grant` and `ungrant`: which server may fetch a key. A grant is a trust decision, so it is
//! confirmed (CMD-024) and has no MCP tool.

use super::documents::{SecretGrant, SecretUngrant, secret_grant, secret_ungrant};
use super::shown::{as_json, record};
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct GrantArgs {
    /// The name of the key.
    name: String,
    /// The ID of the device whose daemon fetches the key.
    #[arg(long, value_name = "ID")]
    device: String,
    /// The workload on that server that uses the key: 1 to 40 of a-z, 0-9, _, . and -.
    #[arg(long)]
    workload: String,
}

impl GrantArgs {
    /// # Errors
    /// [`Failure`] for a declined confirmation or the hub's refusal (`NOT_FOUND`).
    pub async fn grant(self, invocation: &Invocation) -> Result<(), Failure> {
        invocation.prompter.confirm(&format!(
            "Let the daemon of device {} fetch {} for {}?",
            self.device, self.name, self.workload
        ))?;
        let hub = invocation.hub()?;
        let variables = secret_grant::Variables {
            name: self.name,
            device: self.device,
            workload: self.workload,
        };
        let answer = hub.run::<SecretGrant>(variables).await?;
        invocation
            .output
            .data(&as_json(&answer.grant_operational_key), record)
    }

    /// # Errors
    /// [`Failure`] for the hub's refusal (`NOT_FOUND` when there is no such grant).
    pub async fn ungrant(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let variables = secret_ungrant::Variables {
            name: self.name,
            device: self.device,
            workload: self.workload,
        };
        let answer = hub.run::<SecretUngrant>(variables).await?;
        invocation
            .output
            .data(&as_json(&answer.ungrant_operational_key), record)
    }
}
