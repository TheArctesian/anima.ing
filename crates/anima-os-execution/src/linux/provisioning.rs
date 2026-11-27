//! Authorized workspace lifecycle, separate from runner prompts and their journals.
pub(super) mod broker;
mod capacity;
mod receipts;
use super::{Error, Result, workspace};
use agent_threads::managed::workspace::{Action, Command, Phase, Receipt, View};
use anima_os_maintenance::{
    capacity::Resources,
    reservations::{Reservation, RuntimeOwner},
};
use uuid::Uuid;
use workspace::record::Phase as Local;

pub(super) async fn deliver(
    command: Command,
    deadline: tokio::time::Instant,
    checkout: Option<workspace::checkout::ManagedCredential>,
) -> Result<Receipt> {
    if !command.fits() {
        return Err(Error::Refused("Invalid workspace lifecycle request."));
    }
    let store = receipts::Store::open()?;
    if let Some(previous) = store.read(command.request)? {
        if previous.command != command {
            return Err(Error::Refused(
                "This request identity already belongs to another operation.",
            ));
        }
        return Ok(previous);
    }
    if !matches!(command.action, Action::Prepare { .. }) {
        inspect(command.workspace, command.principal)?;
    }
    if tokio::time::Instant::now() >= deadline {
        return Err(Error::Refused("Workspace authorization expired."));
    }
    let mut receipt = Receipt {
        command,
        result: None,
        failure: None,
    };
    store.reserve(&receipt)?;
    // Persist intent first. Cancellation/crash leaves an unresolved receipt, never permission to replay.
    if tokio::time::Instant::now() >= deadline {
        return Ok(receipt);
    }
    match execute(&receipt.command, checkout).await {
        Ok(()) => {
            receipt.result = Some(inspect(
                receipt.command.workspace,
                receipt.command.principal,
            )?);
        }
        Err(error) => {
            receipt.failure = Some(match error {
                Error::Refused(reason) | Error::Admission(anima_os_maintenance::Error::Refused(reason)) => reason.to_owned(),
                Error::WorkspaceBusy => error.to_string(),
                Error::Admission(reason @ (
                    anima_os_maintenance::Error::Busy | anima_os_maintenance::Error::HostingPressure |
                    anima_os_maintenance::Error::PressureUnavailable
                )) => reason.to_string(),
                _ => "Workspace runtime or storage is unavailable; inspect and reconcile before another operation.".to_owned(),
            });
        }
    }
    store.save(&receipt)?;
    Ok(receipt)
}

async fn execute(
    command: &Command,
    checkout: Option<workspace::checkout::ManagedCredential>,
) -> Result<()> {
    match &command.action {
        Action::Delete {
            generation,
            confirmed,
        } => {
            workspace::release(command.workspace, *generation, *confirmed).await?;
        }
        Action::Prepare {
            storage_bytes,
            source,
        } => {
            workspace::prepare_owned(
                command.workspace,
                *storage_bytes,
                Some(command.principal),
                source.clone().map(Into::into),
            )
            .await?;
        }
        Action::Start {
            generation,
            memory_bytes,
            cpu_millis,
            tasks,
        } => {
            workspace::start_managed(
                command.workspace,
                Reservation {
                    schema: 1,
                    runtime: RuntimeOwner::Docker,
                    run: command.request,
                    generation: *generation,
                    resources: Resources {
                        memory_bytes: *memory_bytes,
                        cpu_millis: *cpu_millis,
                        tasks: *tasks,
                    },
                },
                command.principal,
                checkout,
            )
            .await?;
        }
        Action::Stop { generation } => {
            workspace::stop(command.workspace, *generation).await?;
        }
        Action::Reconcile { generation } => {
            workspace::reconcile(command.workspace, *generation).await?;
        }
    }
    Ok(())
}

pub(super) fn status(workspace: Uuid, request: Uuid, principal: Uuid) -> Result<Receipt> {
    let store = receipts::Store::open()?;
    let receipt = store
        .read(request)?
        .ok_or(Error::Refused("Workspace request receipt unavailable."))?;
    if receipt.command.workspace != workspace || receipt.command.principal != principal {
        return Err(Error::Refused(
            "Workspace request belongs to another owner.",
        ));
    }
    Ok(receipt)
}

pub(super) fn inspect(id: Uuid, principal: Uuid) -> Result<View> {
    let _lock = workspace::record::lock(id)?;
    let state = workspace::record::required(id)?;
    if state.principal != Some(principal) {
        return Err(Error::Refused("Workspace belongs to another owner."));
    }
    Ok(View {
        workspace: id,
        checkout: state
            .checkout
            .as_ref()
            .map(workspace::checkout::Receipt::view),
        generation: state.generation,
        storage_bytes: state.storage_bytes,
        phase: match state.phase {
            Local::Preparing => Phase::Preparing,
            Local::Prepared => Phase::Prepared,
            Local::Starting => Phase::Starting,
            Local::Running => Phase::Running,
            Local::Stopping => Phase::Stopping,
            Local::Stopped => Phase::Stopped,
            Local::Unknown => Phase::Unknown,
            Local::Deleting => Phase::Deleting,
            Local::Deleted => Phase::Deleted,
        },
    })
}
