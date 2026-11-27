//! Submit a container action and check its result separately.

use crate::{
    devices::get::resolve, failure::Failure, hub::Hub, invocation::Invocation, output::Table,
};

use super::{
    documents::{
        ContainerActionStatus, QueueContainerAction, container_action_status,
        queue_container_action,
    },
    list::fetch,
    logs::is_identifier,
};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Queue a start on one Docker container.
    Start(Target),
    /// Queue a stop on one Docker container.
    Stop(Target),
    /// Queue a restart on one Docker container.
    Restart(Target),
    /// Read the state of a queued container action.
    ActionStatus(Status),
}

impl Command {
    /// Submit or read an action.
    ///
    /// # Errors
    /// Invalid target, missing action, or a hub refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Start(target) => target.queue(invocation, Verb::Start).await,
            Self::Stop(target) => target.queue(invocation, Verb::Stop).await,
            Self::Restart(target) => target.queue(invocation, Verb::Restart).await,
            Self::ActionStatus(status) => status.read(invocation).await,
        }
    }
}

#[derive(Clone, Copy, Debug, schemars::JsonSchema, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Start,
    Stop,
    Restart,
}

impl From<Verb> for queue_container_action::ContainerActionVerb {
    fn from(verb: Verb) -> Self {
        match verb {
            Verb::Start => Self::START,
            Verb::Stop => Self::STOP,
            Verb::Restart => Self::RESTART,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct Target {
    /// The device's ID or exact name.
    device: String,
    /// The container's exact name, full ID or unique ID prefix of at least 12 digits.
    container: String,
}

impl Target {
    /// Submit the action and print its queued state.
    ///
    /// # Errors
    /// Invalid or ambiguous target, or a hub refusal.
    async fn queue(self, invocation: &Invocation, action: Verb) -> Result<(), Failure> {
        let queued = queue(&invocation.hub()?, (&self.device, &self.container), action).await?;
        invocation.output.data(&queued, |value| {
            record(&value.id, &value.state, value.reason.as_deref())
        })
    }
}

#[derive(Debug, clap::Args)]
pub struct Status {
    /// The action ID printed when the request was queued.
    id: String,
}

impl Status {
    /// Fetch the action's latest state without resubmitting it.
    ///
    /// # Errors
    /// The hub did not answer, or there is no such action.
    async fn read(self, invocation: &Invocation) -> Result<(), Failure> {
        let found = status(&invocation.hub()?, self.id).await?;
        invocation.output.data(&found, |value| {
            record(&value.id, &value.state, value.reason.as_deref())
        })
    }
}

/// Submit a fixed verb for one exact container on one device.
///
/// # Errors
/// Invalid or ambiguous target, or a hub refusal.
pub async fn queue(
    hub: &Hub,
    (device, container): (&str, &str),
    action: Verb,
) -> Result<queue_container_action::QueueContainerActionQueueContainerAction, Failure> {
    let device_id = resolve(hub, device).await?;
    let container_id = full_id(hub, &device_id, container).await?;
    let input = queue_container_action::ContainerActionInput {
        device_id,
        container_id,
        action: action.into(),
    };
    Ok(hub
        .run::<QueueContainerAction>(queue_container_action::Variables { input })
        .await?
        .queue_container_action)
}

/// Get a queued container action by its ID.
///
/// # Errors
/// No such action or a hub refusal.
pub async fn status(
    hub: &Hub,
    id: String,
) -> Result<container_action_status::ContainerActionStatusContainerAction, Failure> {
    hub.run::<ContainerActionStatus>(container_action_status::Variables { id })
        .await?
        .container_action
        .ok_or_else(|| Failure::NotFound {
            message: "No container action has that ID.".into(),
            hint: None,
        })
}

async fn full_id(hub: &Hub, device: &str, text: &str) -> Result<String, Failure> {
    let prefix = is_identifier(text);
    if prefix && text.len() == 64 {
        return Ok(text.to_owned());
    }
    let mut matches = fetch(hub, device).await?.into_iter().filter(|container| {
        container.id.len() == 64
            && is_identifier(&container.id)
            && (container.name == text || (prefix && container.id.starts_with(text)))
    });
    let Some(only) = matches.next() else {
        return Err(Failure::NotFound {
            message: "No container matches this target.".into(),
            hint: None,
        });
    };
    if matches.next().is_some() {
        return Err(Failure::Conflict {
            message: "More than one container matches. Use the full Docker ID.".into(),
        });
    }
    Ok(only.id)
}

fn record(id: &str, state: &str, reason: Option<&str>) -> Table {
    let mut table = Table::record();
    table.pair("Action ID", id);
    table.pair("State", state);
    if let Some(reason) = reason {
        table.pair("Reason", reason);
    }
    table
}
