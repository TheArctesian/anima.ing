//! Closed local request families retain the flat operation-tagged wire format.
use super::{Result, builds, control, previews, workspace};
use anima_os_maintenance::reservations::Reservation;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Request {
    Stream(Stream),
    Operation(Operation),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Operation {
    Build(Build),
    WorkPolicy(super::work_policy::Request),
    SourceJob(super::source_jobs::Request),
    Workspace(Workspace),
    Handoff(workspace::handoff::Request),
    Control(Control),
}

impl Operation {
    pub(super) async fn execute(self) -> Result<Value> {
        match self {
            Self::Build(request) => request.execute().await,
            Self::WorkPolicy(request) => request.execute(),
            Self::SourceJob(request) => request.execute().await,
            Self::Workspace(request) => request.execute().await,
            Self::Handoff(request) => request.execute().await,
            Self::Control(request) => request.execute().await,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Stream {
    WorkspaceHandoffProjectExport {
        receipt: agent_threads::managed::workspace::handoff::Receipt,
    },
    WorkspaceHandoffProjectRestore {
        import: agent_threads::managed::workspace::handoff::Import,
    },
    BuildExport {
        attempt: Uuid,
        archive_digest: String,
        manifest_digest: String,
    },
    Connect {
        workspace: Uuid,
        generation: u64,
    },
    Preview {
        endpoint: Uuid,
        revision: u64,
        principal: Uuid,
    },
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Build {
    #[serde(rename = "build-renew")]
    Renew {
        attempt: Uuid,
        lease: builds::lease::Grant,
    },
    #[serde(rename = "build-logs")]
    Logs { attempt: Uuid },
    #[serde(rename = "build-release")]
    Release { attempt: Uuid, confirmed: bool },
    #[serde(rename = "build-status")]
    Status { attempt: Uuid },
    #[serde(rename = "build-cancel")]
    Cancel { attempt: Uuid },
    #[serde(rename = "build-reconcile")]
    Reconcile { attempt: Uuid },
    #[serde(rename = "build")]
    Start {
        #[serde(default = "builds::default_seconds")]
        seconds: std::num::NonZeroU64,
        #[serde(default)]
        lease: Option<builds::lease::Grant>,
        attempt: Uuid,
        workspace: Uuid,
        generation: u64,
        context: Uuid,
        resources: anima_os_maintenance::capacity::Resources,
        #[serde(default)]
        strategy: Option<builds::worker::Strategy>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Workspace {
    SourceCheckout {
        workspace: Uuid,
        generation: u64,
        source: workspace::checkout::Source,
    },
    #[serde(rename = "workspace-release")]
    Release {
        workspace: Uuid,
        generation: u64,
        confirmed: bool,
    },
    Prepare {
        workspace: Uuid,
        storage_bytes: u64,
    },
    ContextStatus {
        workspace: Uuid,
        generation: u64,
        context: Uuid,
    },
    ContextRelease {
        workspace: Uuid,
        generation: u64,
        context: Uuid,
        confirmed: bool,
    },
    Capture(workspace::context::Identity),
    Plan {
        workspace: Uuid,
        generation: u64,
        preparation: Uuid,
    },
    Start {
        workspace: Uuid,
        reservation: Reservation,
    },
    Reconcile {
        workspace: Uuid,
        generation: u64,
    },
    Status {
        workspace: Uuid,
    },
    Stop {
        workspace: Uuid,
        generation: u64,
    },
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Control {
    Inventory {
        #[serde(default)]
        usage: bool,
    },
    CodeAttention {},
    CodeCommand {
        command: control::Command,
    },
    CodeCommandStatus {
        workspace: Uuid,
        request: Uuid,
        principal: Uuid,
    },
    PreviewOpen {
        binding: previews::Binding,
    },
    PreviewRevoke {
        endpoint: Uuid,
        revision: u64,
    },
}

impl Build {
    async fn execute(self) -> Result<Value> {
        match self {
            Self::Renew { attempt, lease } => builds::lease::renew(attempt, lease),
            Self::Logs { attempt } => builds::logs(attempt),
            Self::Release { attempt, confirmed } => builds::release(attempt, confirmed).await,
            Self::Status { attempt } => builds::status(attempt).await,
            Self::Cancel { attempt } => builds::cancel(attempt).await,
            Self::Reconcile { attempt } => builds::reconcile(attempt).await,
            Self::Start {
                seconds,
                lease,
                attempt,
                workspace,
                generation,
                context,
                resources,
                strategy,
            } => {
                builds::start(
                    attempt,
                    workspace::context::Identity {
                        workspace,
                        generation,
                        context,
                    },
                    builds::Choice {
                        supervisor: None,
                        seconds,
                        lease,
                        resources,
                        strategy,
                    },
                )
                .await
            }
        }
    }
}

impl Workspace {
    async fn execute(self) -> Result<Value> {
        match self {
            Self::SourceCheckout {
                workspace,
                generation,
                source,
            } => workspace::checkout::run(workspace, generation, source).await,
            Self::Release {
                workspace,
                generation,
                confirmed,
            } => workspace::release(workspace, generation, confirmed).await,
            Self::Prepare {
                workspace,
                storage_bytes,
            } => workspace::prepare(workspace, storage_bytes).await,
            Self::ContextStatus {
                workspace,
                generation,
                context,
            } => workspace::context::status(workspace, generation, context),
            Self::ContextRelease {
                workspace,
                generation,
                context,
                confirmed,
            } => workspace::context::release(workspace, generation, context, confirmed).await,
            Self::Capture(identity) => workspace::context::capture(identity, None).await,
            Self::Plan {
                workspace,
                generation,
                preparation,
            } => workspace::planning::prepare(workspace, generation, preparation).await,
            Self::Start {
                workspace,
                reservation,
            } => workspace::start(workspace, reservation).await,
            Self::Reconcile {
                workspace,
                generation,
            } => workspace::reconcile(workspace, generation).await,
            Self::Status { workspace } => workspace::status(workspace).await,
            Self::Stop {
                workspace,
                generation,
            } => workspace::stop(workspace, generation).await,
        }
    }
}

impl Control {
    async fn execute(self) -> Result<Value> {
        match self {
            Self::Inventory { usage } => Ok(control::inventory::read(usage).await),
            Self::CodeAttention {} => control::attention::read().await,
            Self::CodeCommand { command } => control::deliver(command).await,
            Self::CodeCommandStatus {
                workspace,
                request,
                principal,
            } => control::status(workspace, request, principal).await,
            Self::PreviewOpen { binding } => previews::open(binding).await,
            Self::PreviewRevoke { endpoint, revision } => previews::revoke(endpoint, revision),
        }
    }
}

#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
