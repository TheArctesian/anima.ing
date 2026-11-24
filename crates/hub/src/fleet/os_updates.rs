//! Owner-requested signed OS updates over the device's pinned private SSH connection.

#![expect(
    clippy::unused_async_trait_impl,
    reason = "async-graphql Enum derives async trait implementations"
)]

pub mod graphql;

use std::time::Duration;

use async_graphql::Enum;
use serde_json::Value;

use crate::fleet::ssh::{FixedCommand, SshLink};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum OsUpdateAction {
    Check,
    Download,
    Activate,
    Rollback,
}

impl OsUpdateAction {
    const fn command(self) -> &'static str {
        match self {
            Self::Check => "remote-check",
            Self::Download => "remote-download",
            Self::Activate => "remote-activate",
            Self::Rollback => "remote-rollback",
        }
    }

    const fn restarts(self) -> bool {
        matches!(self, Self::Activate | Self::Rollback)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Link(#[from] crate::fleet::ssh::LinkError),
    #[error(
        "This device cannot provide remote OS updates. Install an OS image with remote update controls and check SSH access."
    )]
    Unsupported,
    #[error("The device returned an invalid update response.")]
    Invalid,
}

async fn run(link: &mut SshLink, action: Option<OsUpdateAction>) -> Result<Value, Error> {
    let command = FixedCommand::new("sudo")
        .arg("-n")
        .arg("/run/current-system/sw/bin/anima-os-update")
        .arg(action.map_or("remote-status", OsUpdateAction::command));
    let output = link.run(&command, Duration::from_secs(20)).await?;
    if output.cut || output.stdout.len() > 65536 {
        return Err(Error::Invalid);
    }
    if output.exit_status != Some(0) {
        return Err(Error::Unsupported);
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| Error::Invalid)?;
    if value["ok"] != true || !value["value"].is_object() {
        return Err(Error::Invalid);
    }
    Ok(value["value"].clone())
}
