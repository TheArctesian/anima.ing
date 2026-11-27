//! Submit Compose files from the operator's terminal and read an apply's public result.
use std::path::{Path, PathBuf};

use zeroize::{Zeroize as _, Zeroizing};

use crate::{
    devices::{credentials::input::send_wiping, get::resolve},
    failure::{Failure, Hint},
    hub::{Hub, TokenKind},
    input,
    invocation::Invocation,
    output::Table,
};

use super::{
    check_file::LARGEST_FILE,
    documents::{
        ApplicationApplyStatus, ApplicationApplyTargets, QueueApplicationApply,
        application_apply_status, application_apply_targets, queue_application_apply,
    },
};

const LARGEST_ENVIRONMENT: usize = 64 * 1024;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Submit a managed Compose application as the signed-in operator.
    Apply(Apply),
    /// Show the last state and health of a managed Compose apply.
    ApplyStatus(Status),
    /// List devices that currently accept Compose applies.
    ApplyTargets,
}

impl Command {
    pub(super) async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Apply(args) => args.run(invocation).await,
            Self::ApplyStatus(args) => args.run(invocation).await,
            Self::ApplyTargets => targets(invocation).await,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct Apply {
    /// An enrolled device's ID or exact name.
    device: String,
    /// A Compose file, or `-` for standard input.
    compose_file: PathBuf,
    /// Optional environment file. Use a path or `-` for standard input.
    #[arg(long)]
    env_file: Option<PathBuf>,
    /// Application ID from a previous apply, to update it.
    #[arg(long)]
    application: Option<String>,
    /// Reuse the application's recorded image digests.
    #[arg(long, requires = "application")]
    no_pull: bool,
    /// Discard the previous version after the apply.
    #[arg(long)]
    discard_previous: bool,
}

#[derive(Debug, clap::Args)]
pub struct Status {
    /// Apply ID printed when the apply was queued.
    id: String,
}

impl Apply {
    /// Read the files and queue the apply as the signed-in operator.
    ///
    /// # Errors
    /// Invalid device or files, or a refusal from the hub.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        if hub.declares_agent()
            || hub
                .token()
                .is_none_or(|token| token.kind() != TokenKind::Login)
        {
            return Err(Failure::Forbidden {
                message: "A login is required to submit Compose and environment files.".to_owned(),
                hint: Some(Hint::run("anima auth login")),
            });
        }
        let input = self.input(invocation, &hub).await?;
        let answer = send_wiping::<QueueApplicationApply>(
            &hub,
            queue_application_apply::Variables { input },
            |variables| {
                variables.input.compose_file.zeroize();
                variables.input.environment_file.zeroize();
            },
        )
        .await?
        .queue_application_apply;
        invocation.output.data(&answer, |value| {
            record(
                (
                    &value.id,
                    &value.application_id,
                    &value.device_id,
                    &value.state,
                ),
                (value.reason.as_deref(), value.health.as_deref()),
            )
        })
    }
    async fn input(
        self,
        invocation: &Invocation,
        hub: &Hub,
    ) -> Result<queue_application_apply::ApplicationApplyInput, Failure> {
        if self.compose_file.as_os_str() == "-"
            && self
                .env_file
                .as_deref()
                .is_some_and(|path| path.as_os_str() == "-")
        {
            return Err(Failure::Usage {
                message: "Use standard input for only one file.".to_owned(),
                hint: None,
            });
        }
        let device_id = resolve(hub, &self.device).await?;
        let compose = text(
            invocation,
            &self.compose_file,
            "the Compose file",
            LARGEST_FILE,
        )?;
        let environment = self
            .env_file
            .as_deref()
            .map(|path| {
                text(
                    invocation,
                    path,
                    "the environment file",
                    LARGEST_ENVIRONMENT,
                )
            })
            .transpose()?;
        Ok(queue_application_apply::ApplicationApplyInput {
            device_id,
            application_id: self.application,
            compose_file: compose.to_string(),
            environment_file: environment.as_deref().map(ToString::to_string),
            pull: !self.no_pull,
            keep_previous: !self.discard_previous,
        })
    }
}

impl Status {
    /// Read the latest status of this apply.
    ///
    /// # Errors
    /// Missing apply or a refusal from the hub.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let answer = status(&invocation.hub()?, self.id).await?;
        invocation.output.data(&answer, |value| {
            record(
                (
                    &value.id,
                    &value.application_id,
                    &value.device_id,
                    &value.state,
                ),
                (value.reason.as_deref(), value.health.as_deref()),
            )
        })
    }
}

/// Read the apply's public result without requesting its files.
///
/// # Errors
/// Missing apply or a refusal from the hub.
pub async fn status(
    hub: &Hub,
    id: String,
) -> Result<application_apply_status::ApplicationApplyStatusApplicationApply, Failure> {
    hub.run::<ApplicationApplyStatus>(application_apply_status::Variables { id })
        .await?
        .application_apply
        .ok_or_else(|| Failure::NotFound {
            message: "No Compose apply has that ID.".to_owned(),
            hint: None,
        })
}

/// Read the devices advertising Compose apply support.
///
/// # Errors
/// A refusal from the hub.
pub async fn list_targets(
    hub: &Hub,
) -> Result<Vec<application_apply_targets::ApplicationApplyTargetsApplicationApplyTargets>, Failure>
{
    Ok(hub
        .run::<ApplicationApplyTargets>(application_apply_targets::Variables)
        .await?
        .application_apply_targets)
}

/// Print the devices that currently accept Compose applies.
///
/// # Errors
/// A refusal from the hub.
pub async fn targets(invocation: &Invocation) -> Result<(), Failure> {
    let devices = list_targets(&invocation.hub()?).await?;
    invocation.output.data(&devices, |list| {
        let mut table = Table::new(vec!["DEVICE", "ID"]);
        for device in list {
            table.row(vec![device.name.as_str().into(), device.id.as_str().into()]);
        }
        table
    })
}

fn text(
    invocation: &Invocation,
    path: &Path,
    what: &str,
    limit: usize,
) -> Result<Zeroizing<String>, Failure> {
    let bytes = input::read(invocation.prompter, path, what, limit)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Failure::Input {
        what: what.to_owned(),
        reason: "it is not UTF-8 text".to_owned(),
    })?;
    Ok(Zeroizing::new(text.to_owned()))
}

fn record(
    (id, application, device, state): (&str, &str, &str, &str),
    (reason, health): (Option<&str>, Option<&str>),
) -> Table {
    let mut table = Table::record();
    table.pair("Apply ID", id);
    table.pair("Application ID", application);
    table.pair("Device ID", device);
    table.pair("State", state);
    if let Some(reason) = reason {
        table.pair("Reason", reason);
    }
    if let Some(health) = health {
        table.pair("Health", health);
    }
    match reason {
        Some("secret_failed") => table.pair("Next step", "Submit a fresh apply with both files."),
        Some("migration_held") => table.pair(
            "Next step",
            "Complete the local Podman cutover checks on the device.",
        ),
        Some("orphan") => table.pair(
            "Next step",
            "Check the application before submitting another apply.",
        ),
        _ => (),
    }
    table
}
