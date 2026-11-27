//! Workspace lifecycle commands never issue arbitrary host commands or receive credentials.
use super::{
    documents::{
        WorkspaceInventory, WorkspaceReceipt, WorkspaceSubmit, workspace_inventory,
        workspace_receipt, workspace_submit,
    },
    record,
};
use crate::{devices::get::resolve, failure::Failure, invocation::Invocation};
use agent_threads::managed::workspace::{Action, Source};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, clap::Args)]
pub struct Target {
    pub device: String,
    pub workspace: Uuid,
    /// Reuse only with the identical action after an unconfirmed submission.
    #[arg(long)]
    pub request: Option<Uuid>,
}
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Delete stopped or never-started workspace files after releasing its builds and contexts.
    Delete {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        generation: u64,
        /// Confirm permanent removal of this workspace's files.
        #[arg(long, required = true)]
        confirm: bool,
    },
    /// List this person's workspace lifecycle records and node availability.
    List { device: String },
    /// Read a lifecycle receipt; unresolved never means safe to repeat.
    Status { request: Uuid },
    /// Prepare persistent, quota-limited storage; does not start an agent.
    Prepare {
        #[command(flatten)]
        target: Target,
        #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..=64))]
        storage_gib: u64,
        /// GitHub owner/repository allowed by the node.
        #[arg(long, requires = "commit")]
        repository: Option<String>,
        /// Full commit SHA to check out on first start.
        #[arg(long, requires = "repository")]
        commit: Option<String>,
        /// Connected Projects repository ID for a private checkout.
        #[arg(long, requires_all = ["repository", "commit"])]
        repository_id: Option<Uuid>,
    },
    /// Start the next exact workspace generation within hosting-first admission limits.
    Start {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        generation: u64,
        #[arg(long, default_value_t = 1024, value_parser = clap::value_parser!(u64).range(64..=65536))]
        memory_mib: u64,
        #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(1..=64000))]
        cpu_millis: u64,
        #[arg(long, default_value_t = 256, value_parser = clap::value_parser!(u64).range(1..=512))]
        tasks: u64,
    },
    /// Stop all work in this exact generation, retaining persistent files.
    Stop {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        generation: u64,
    },
    /// Reconcile an uncertain generation against the node runtime without relaunching it.
    Reconcile {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        generation: u64,
    },
}
impl Command {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        match self {
            Self::List { device } => {
                let device = resolve(&hub, &device).await?;
                let reply = hub
                    .run::<WorkspaceInventory>(workspace_inventory::Variables { device })
                    .await?;
                invocation
                    .output
                    .data(&reply.managed_node_workspaces, record)
            }
            Self::Status { request } => {
                let reply = hub
                    .run::<WorkspaceReceipt>(workspace_receipt::Variables {
                        request: request.to_string(),
                    })
                    .await?;
                invocation
                    .output
                    .data(&reply.managed_workspace_request, |value| {
                        record(value.as_ref().unwrap_or(&serde_json::Value::Null))
                    })
            }
            mutation => {
                let (target, action) = action(mutation)?;
                let device = resolve(&hub, &target.device).await?;
                let request = target.request.unwrap_or_else(Uuid::now_v7);
                let input = workspace_submit::ManagedWorkspaceInput {
                    request: request.to_string(),
                    workspace: target.workspace.to_string(),
                    action: serde_json::to_value(action).map_err(|_| invalid())?,
                };
                let reply = hub.run::<WorkspaceSubmit>(workspace_submit::Variables { device, input }).await
                    .map_err(|error| Failure::NotDelivered { message: format!("Workspace submission was not confirmed: {error}. Inspect anima code node workspace status {request}; retry only the identical action with --request {request}.") })?;
                invocation.output.data(&json!({"request":reply.queue_managed_workspace_request,"workspace":target.workspace,"submission":"accepted","executionConfirmed":false}), record)
            }
        }
    }
}
fn action(command: Command) -> Result<(Target, Action), Failure> {
    match command {
        Command::Delete {
            target,
            generation,
            confirm,
        } => Ok((
            target,
            Action::Delete {
                generation,
                confirmed: confirm,
            },
        )),
        Command::Prepare {
            target,
            storage_gib,
            repository,
            commit,
            repository_id,
        } => Ok((
            target,
            Action::Prepare {
                storage_bytes: storage_gib << 30,
                source: checkout_source(repository.as_deref(), commit.as_deref(), repository_id)?,
            },
        )),
        Command::Start {
            target,
            generation,
            memory_mib,
            cpu_millis,
            tasks,
        } => Ok((
            target,
            Action::Start {
                generation,
                memory_bytes: memory_mib << 20,
                cpu_millis,
                tasks,
            },
        )),
        Command::Stop { target, generation } => Ok((target, Action::Stop { generation })),
        Command::Reconcile { target, generation } => Ok((target, Action::Reconcile { generation })),
        _ => Err(invalid()),
    }
}
fn checkout_source(
    repository: Option<&str>,
    commit: Option<&str>,
    repository_id: Option<Uuid>,
) -> Result<Option<Source>, Failure> {
    if repository.is_none() && commit.is_none() && repository_id.is_none() {
        return Ok(None);
    }
    if let (Some((owner, repository)), Some(commit)) =
        (repository.and_then(|name| name.split_once('/')), commit)
    {
        let source = Source {
            owner: owner.to_owned(),
            repository: repository.to_owned(),
            commit: commit.to_ascii_lowercase(),
            repository_id,
        };
        if source.fits() {
            return Ok(Some(source));
        }
    }
    Err(Failure::Usage {
        message:
            "Use --repository OWNER/REPOSITORY and --commit FULL_SHA. For a private checkout, also supply its connected --repository-id."
                .to_owned(),
        hint: None,
    })
}

fn invalid() -> Failure {
    Failure::Usage {
        message: "Choose a workspace lifecycle action.".to_owned(),
        hint: None,
    }
}
