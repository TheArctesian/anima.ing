//! Person-operated managed Code commands through the hub, never a direct root connection.
mod args;
pub(crate) mod documents;
mod login;
mod previews;
mod workspaces;
use crate::{
    devices::get::resolve,
    failure::Failure,
    hub::Hub,
    invocation::Invocation,
    output::{Cell, Table},
};
pub use args::Command;
use documents::{
    ManagedEnable, ManagedReceipt, ManagedSubmit, ManagedWorkspaces, managed_enable,
    managed_receipt, managed_submit, managed_workspaces,
};
use serde_json::{Value, json};
use uuid::Uuid;

impl Command {
    /// Runs a person-operated managed Code command through the hub.
    ///
    /// # Errors
    /// Returns input or hub errors, and an explicit unresolved-submission error when
    /// delivery cannot be confirmed. Output errors are propagated.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        match self {
            Self::Login {
                provider,
                device,
                workspace,
                generation,
            } => login::run(invocation, (device, workspace, generation), provider).await,
            Self::Preview { command } => command.run(invocation).await,
            Self::Workspace { command } => command.run(invocation).await,
            Self::Workspaces { device } => inventory(invocation, &hub, &device).await,
            Self::Status { request } => {
                let receipt = hub
                    .run::<ManagedReceipt>(managed_receipt::Variables {
                        request: request.to_string(),
                    })
                    .await?
                    .managed_code_command;
                invocation.output.data(&receipt, |value| {
                    record(value.as_ref().unwrap_or(&Value::Null))
                })
            }
            Self::Enable {
                device,
                workspace,
                off,
            } => {
                let device = resolve(&hub, &device).await?;
                let enabled = hub
                    .run::<ManagedEnable>(managed_enable::Variables {
                        device,
                        workspace: workspace.to_string(),
                        enabled: !off,
                    })
                    .await?
                    .set_managed_code_workspace_enabled;
                let value = json!({"workspace":workspace,"enabled":enabled});
                invocation.output.data(&value, record)
            }
            mutation => submit(invocation, &hub, mutation).await,
        }
    }
}
fn cell(value: &Value) -> Cell {
    let text = value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned);
    Cell::new(&text)
}

fn record(value: &Value) -> Table {
    let mut table = Table::new(vec!["FIELD", "VALUE"]);
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            table.row(vec![Cell::new(key), cell(value)]);
        }
    } else {
        table.row(vec![Cell::new("receipt"), cell(value)]);
    }
    table
}

async fn inventory(invocation: &Invocation, hub: &Hub, device: &str) -> Result<(), Failure> {
    let device = resolve(hub, device).await?;
    let rows = hub
        .run::<ManagedWorkspaces>(managed_workspaces::Variables { device })
        .await?
        .managed_code_workspaces;
    invocation.output.data(&rows, |rows| {
        let mut table = Table::new(vec!["WORKSPACE", "GENERATION", "ENABLED", "READY"]);
        for row in rows {
            table.row(
                ["workspace", "generation", "enabled", "ready"]
                    .into_iter()
                    .map(|key| cell(&row[key]))
                    .collect(),
            );
        }
        table
    })
}

async fn submit(invocation: &Invocation, hub: &Hub, command: Command) -> Result<(), Failure> {
    let (target, action) = args::mutation(command)?;
    let device = resolve(hub, &target.device).await?;
    let request = target.request.unwrap_or_else(Uuid::now_v7);
    let action = serde_json::to_value(action).map_err(|_| Failure::Usage {
        message: "Invalid Code action.".into(),
        hint: None,
    })?;
    let queued = hub.run::<ManagedSubmit>(managed_submit::Variables { device,
                    input:managed_submit::ManagedCodeInput { request:request.to_string(),workspace:target.workspace.to_string(),
                        generation:target.generation.to_string(),action } }).await
                    .map_err(|error| Failure::NotDelivered { message:format!("Submission was not confirmed: {error}. Inspect anima code node status {request}; retry only the same command with --request {request}.") })?;
    let value = json!({"request":queued.queue_managed_code_command,"submission":"accepted","executionConfirmed":false});
    invocation.output.data(&value, record)
}
