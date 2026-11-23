//! Listing checks over the SSH link, one command per attempt (D-3). The output is never logged
//! or stored; a link error's text holds no host, address or secret (LOG-007).

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::{
    docker,
    outcome::{Failure, ProbeResult},
    systemd, within,
};
use crate::fleet::{
    checks::config::CheckConfig,
    credentials::Vault,
    devices::Device,
    ssh::{self, CommandOutput, FixedCommand, LinkError},
    targets::AddressPolicy,
};

pub(in crate::fleet::checks) struct Reach<'a> {
    pub(in crate::fleet::checks) db: &'a PgPool,
    pub(in crate::fleet::checks) vault: &'a Vault,
    pub(in crate::fleet::checks) policy: &'a AddressPolicy,
}

pub(super) fn readable(output: &CommandOutput) -> Result<&str, Failure> {
    if output.cut {
        return Err(Failure::OutputCut);
    }
    if output.exit_status != Some(0) {
        return Err(Failure::CommandFailed);
    }
    std::str::from_utf8(&output.stdout).map_err(|_| Failure::OutputUnreadable)
}

fn link_failure(error: &LinkError) -> Failure {
    match error {
        LinkError::TimeLimit => Failure::TimedOut,
        other => Failure::Link(other.to_string()),
    }
}

async fn run(
    reach: &Reach<'_>,
    device: &Device,
    command: &FixedCommand,
    (limit, now): (std::time::Duration, DateTime<Utc>),
) -> Result<CommandOutput, Failure> {
    let mut link = ssh::link(reach.db, reach.vault, reach.policy, device, now)
        .await
        .map_err(|error| link_failure(&error))?;
    link.run(command, limit)
        .await
        .map_err(|error| link_failure(&error))
}

pub(in crate::fleet::checks) async fn observe_listing(
    reach: &Reach<'_>,
    device: &Device,
    config: &CheckConfig,
    now: DateTime<Utc>,
) -> ProbeResult {
    let (command, read, name): (FixedCommand, fn(&CommandOutput, &str) -> ProbeResult, &str) =
        match config {
            CheckConfig::Docker { container, .. } => (docker::command(), docker::read, container),
            CheckConfig::Systemd { unit, .. } => (systemd::command(), systemd::read, unit),
            _ => return ProbeResult::failed(&Failure::NotProbed),
        };
    let attempt = async {
        match run(reach, device, &command, (config.timeout(), now)).await {
            Ok(output) => read(&output, name),
            Err(failure) => ProbeResult::failed(&failure),
        }
    };
    within(config, attempt).await
}
