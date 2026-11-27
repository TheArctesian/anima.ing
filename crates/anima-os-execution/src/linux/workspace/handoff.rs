//! A stopped source retains its storage and can never launch while it owns a handoff.
pub(in crate::linux) mod project;
pub(in crate::linux) mod restore;
mod withdrawal;

use super::{
    Error, Result,
    record::{self, Phase, Record},
};
pub(super) use agent_threads::managed::workspace::handoff::Binding;
use agent_threads::managed::workspace::handoff::{Import, Source, valid_generation};
use anima_os_maintenance::Admission;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub(in crate::linux) enum Request {
    #[serde(rename = "workspace-handoff-fence")]
    Fence {
        workspace: Uuid,
        generation: u64,
        principal: Uuid,
        binding: Binding,
    },
    #[serde(rename = "workspace-handoff-status")]
    Status {
        workspace: Uuid,
        generation: u64,
        principal: Uuid,
        transfer: Uuid,
    },
    #[serde(rename = "workspace-handoff-project-describe")]
    DescribeProject { source: Source },
    #[serde(rename = "workspace-handoff-project-restore-status")]
    RestoreStatus { import: Import },
    #[serde(rename = "workspace-handoff-project-activate")]
    Activate { import: Import },
    #[serde(rename = "workspace-handoff-discard")]
    Discard { import: Import },
    #[serde(rename = "workspace-handoff-withdraw")]
    Withdraw { source: Source },
    #[serde(rename = "workspace-handoff-retire")]
    Retire { source: Source },
}

impl Request {
    pub(in crate::linux) async fn execute(self) -> Result<Value> {
        match self {
            Self::Fence {
                workspace,
                generation,
                principal,
                binding,
            } => fence(workspace, generation, principal, binding).await,
            Self::Status {
                workspace,
                generation,
                principal,
                transfer,
            } => status(workspace, generation, principal, transfer),
            Self::DescribeProject { source } => project::describe(source).await,
            Self::RestoreStatus { import } => restore::status(&import),
            Self::Activate { import } => restore::activate(&import),
            Self::Discard { import } => restore::discard(&import).await,
            Self::Withdraw { source } => withdrawal::withdraw(source).await,
            Self::Retire { source } => super::cleanup::retire(source).await,
        }
    }
}

pub(super) fn validate_record(binding: Binding, state: &Record) -> Result<()> {
    if binding.fits()
        && state.principal.is_some_and(|principal| !principal.is_nil())
        && valid_generation(state.generation)
        && state.run.is_some_and(|run| !run.is_nil())
        && matches!(
            state.phase,
            Phase::Stopping | Phase::Stopped | Phase::Deleting | Phase::Deleted
        )
    {
        Ok(())
    } else {
        Err(Error::Refused(
            "Workspace handoff state needs reconciliation.",
        ))
    }
}

async fn fence(
    workspace: Uuid,
    generation: u64,
    principal: Uuid,
    binding: Binding,
) -> Result<Value> {
    let admission = Admission::cleanup()?;
    let _workspace = record::lock(workspace)?;
    let mut state = record::required(workspace)?;
    withdrawal::require_open(
        Source {
            workspace,
            generation,
            principal,
            binding,
        },
        withdrawal::Side::Source,
    )?;
    if record_intent(&mut state, generation, principal, binding)? {
        record::save(&state)?;
    }
    let stopped = tokio::time::timeout(
        Duration::from_secs(60),
        super::stop_locked(state, &admission),
    )
    .await
    .map_err(|_| Error::Refused("The handoff is waiting for source termination."))??;
    Ok(view(&stopped, binding))
}

fn status(workspace: Uuid, generation: u64, principal: Uuid, transfer: Uuid) -> Result<Value> {
    let state = record::required(workspace)?;
    authorize(&state, generation, principal)?;
    let binding = state
        .handoff
        .filter(|binding| binding.transfer == transfer)
        .ok_or(Error::Refused("The source has no matching handoff."))?;
    Ok(view(&state, binding))
}

fn record_intent(
    state: &mut Record,
    generation: u64,
    principal: Uuid,
    binding: Binding,
) -> Result<bool> {
    authorize(state, generation, principal)?;
    if let Some(previous) = state.handoff {
        if previous != binding {
            return Err(Error::Refused(
                "The workspace already belongs to another handoff.",
            ));
        }
        return Ok(false);
    }
    if !binding.fits() || !valid_source(state) {
        return Err(Error::Refused(
            "The workspace cannot begin a handoff in its current state.",
        ));
    }
    state.handoff = Some(binding);
    state.phase = Phase::Stopping;
    Ok(true)
}

fn authorize(state: &Record, generation: u64, principal: Uuid) -> Result<()> {
    if principal.is_nil()
        || state.principal != Some(principal)
        || state.generation != generation
        || !valid_generation(generation)
    {
        return Err(Error::Refused(
            "The handoff does not match the workspace owner and generation.",
        ));
    }
    Ok(())
}

fn valid_source(state: &Record) -> bool {
    state.run.is_some_and(|run| !run.is_nil())
        && matches!(
            state.phase,
            Phase::Starting | Phase::Running | Phase::Stopping | Phase::Stopped | Phase::Unknown
        )
}

fn view(state: &Record, binding: Binding) -> Value {
    json!({
        "workspace":state.workspace,
        "generation":state.generation,
        "run":state.run,
        "binding":binding,
        "state":if state.phase == Phase::Stopped { "fenced" } else { "stopping" },
    })
}

#[cfg(test)]
mod tests;
