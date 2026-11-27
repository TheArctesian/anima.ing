pub(super) mod checkout;
pub(super) mod cleanup;
pub use cleanup::release;
pub(super) mod handoff;
mod inventory;
pub use inventory::list as inventory;
pub(super) use inventory::with_usage as inventory_with_usage;
pub(super) mod context;
pub(super) mod planning;
pub(super) mod record;
mod recovery;
mod runtime;
pub(super) mod servers;
mod start;
pub use recovery::reconcile;
pub use start::start;
pub(super) use start::{start_managed, start_source};

use super::{Error, Result, command};
use anima_os_maintenance::{Admission, reservations, source_pipeline::Pipeline};
use record::{Phase, Record};
use serde_json::{Value, json};
use uuid::Uuid;

pub async fn prepare(id: Uuid, storage_bytes: u64) -> Result<Value> {
    prepare_owned(id, storage_bytes, None, None).await
}

pub(super) async fn prepare_owned(
    id: Uuid,
    storage_bytes: u64,
    principal: Option<Uuid>,
    source: Option<checkout::Source>,
) -> Result<Value> {
    prepare_with(id, storage_bytes, (principal, source), None).await
}

pub(super) async fn prepare_source(pipeline: Pipeline, storage_bytes: u64) -> Result<Value> {
    prepare_with(
        pipeline.workspace(),
        storage_bytes,
        (None, None),
        Some(pipeline),
    )
    .await
}

async fn prepare_with(
    id: Uuid,
    storage_bytes: u64,
    (principal, source): (Option<Uuid>, Option<checkout::Source>),
    pipeline: Option<Pipeline>,
) -> Result<Value> {
    let admission = Admission::for_pipeline(pipeline)?;
    let allocation = record::allocation()?;
    let _lock = record::lock(id)?;
    if !(1024_u64.pow(3)..=64 * 1024_u64.pow(3)).contains(&storage_bytes) {
        return Err(Error::Refused(
            "Choose a workspace quota between 1 and 64 GiB.",
        ));
    }
    if let Some(existing) = record::read(id)? {
        return prepared(existing, storage_bytes, principal, source.as_ref()).await;
    }
    admission.require_open_work()?;
    if !has_room()? {
        return Err(Error::Refused(
            "This node's workspace inventory is full. Choose another node.",
        ));
    }
    if record::free_storage(&allocation)? < storage_bytes {
        return Err(Error::Refused(
            "Hosting reserves the remaining disk space. Choose another node.",
        ));
    }
    let mut state = Record::new(id, storage_bytes);
    state.principal = principal;
    if let Some(source) = source {
        source.authorize()?;
        state.checkout = Some(checkout::Receipt::pending(source));
    }
    record::save(&state)?;
    recovery::finish_preparing(state).await
}

pub(super) fn has_room() -> Result<bool> {
    let inventory = inventory()?;
    Ok(inventory["truncated"] == false
        && inventory["entries"]
            .as_array()
            .is_some_and(|entries| entries.len() < 128))
}

async fn prepared(
    existing: Record,
    storage_bytes: u64,
    principal: Option<Uuid>,
    source: Option<&checkout::Source>,
) -> Result<Value> {
    if existing.incoming.is_some() {
        return Err(Error::Refused("This workspace was reserved by a handoff."));
    }
    if matches!(existing.phase, Phase::Deleting | Phase::Deleted) {
        return Err(Error::Refused(
            "This workspace is being deleted or has been deleted. Choose a new identity.",
        ));
    }
    if existing.principal != principal
        || existing.storage_bytes != storage_bytes
        || (principal.is_some()
            && existing.checkout.as_ref().map(checkout::Receipt::source) != source)
    {
        return Err(Error::Refused(
            "The workspace preparation needs reconciliation.",
        ));
    }
    if existing.phase == Phase::Preparing {
        return recovery::finish_preparing(existing).await;
    }
    Ok(serde_json::to_value(existing)?)
}

pub async fn status(id: Uuid) -> Result<Value> {
    let _lock = record::lock(id)?;
    let state = record::required(id)?;
    if matches!(
        state.phase,
        Phase::Prepared | Phase::Preparing | Phase::Stopped | Phase::Deleting | Phase::Deleted
    ) {
        return Ok(serde_json::to_value(state)?);
    }
    runtime::observe(&state).await
}

pub async fn stop(id: Uuid, generation: u64) -> Result<Value> {
    let admission = anima_os_maintenance::Admission::cleanup()?;
    let _lock = record::lock(id)?;
    let state = record::required(id)?;
    if state.generation != generation {
        return Err(Error::Refused("The workspace generation changed."));
    }
    if state.phase == Phase::Stopped {
        return Ok(serde_json::to_value(state)?);
    }
    let state = stop_locked(state, &admission).await?;
    Ok(json!({"workspace":state,"dataRetained":true}))
}

async fn stop_locked(mut state: Record, admission: &Admission) -> Result<Record> {
    if matches!(state.phase, Phase::Deleting | Phase::Deleted) {
        return Err(Error::Refused(
            "Retry deleting this workspace instead of stopping it.",
        ));
    }
    if state.phase == Phase::Stopped {
        return Ok(state);
    }
    if state.phase != Phase::Stopping {
        if matches!(state.phase, Phase::Prepared | Phase::Preparing) {
            return Err(Error::Refused("This workspace has no running generation."));
        }
        state.phase = Phase::Stopping;
        record::save(&state)?;
    }
    runtime::stop(&state).await?;
    if state.handoff.is_some() {
        cleanup::absent_runtime(state.workspace).await?;
    }
    if let Some(run) = state.run {
        reservations::release_stopped(admission, run, state.generation).await?;
    }
    state.phase = Phase::Stopped;
    record::save(&state)?;
    Ok(state)
}

pub async fn connection(id: Uuid, generation: u64) -> Result<String> {
    let _lock = record::lock(id)?;
    let state = running_record(id, generation)?;
    let observed = runtime::observe(&state).await?;
    if observed["runtime"]["running"] != true || observed["runnerReady"] != true {
        return Err(Error::Refused(
            "The workspace is stopped. Reconcile it before starting a new generation.",
        ));
    }
    Ok(state.container())
}

pub(super) async fn attention_target(id: Uuid, generation: u64) -> Result<String> {
    let state = running_record(id, generation)?;
    if runtime::inspect(&state).await?["running"] != true {
        return Err(Error::Refused("The workspace is no longer running."));
    }
    Ok(state.container())
}

fn running_record(id: Uuid, generation: u64) -> Result<Record> {
    let state = record::required(id)?;
    if state.generation != generation || state.phase != Phase::Running || !state.checkout_ready() {
        return Err(Error::Refused(
            "The requested workspace generation is not running.",
        ));
    }
    Ok(state)
}

pub(super) fn check_preview(id: Uuid, generation: u64, principal: Uuid) -> Result<()> {
    let state = running_record(id, generation)?;
    if state.principal.is_some_and(|owner| owner != principal) {
        return Err(Error::Refused("This workspace belongs to another person."));
    }
    Ok(())
}

pub(super) async fn stop_if_present(id: Uuid, generation: u64) -> Result<()> {
    let state = {
        let _lock = record::lock(id)?;
        record::read(id)?
    };
    let Some(state) = state else { return Ok(()) };
    if state.principal.is_some() || state.generation > generation {
        return Err(Error::Refused(
            "The source workspace has conflicting ownership.",
        ));
    }
    if matches!(
        state.phase,
        Phase::Prepared | Phase::Preparing | Phase::Deleted
    ) {
        return Ok(());
    }
    stop(id, state.generation).await.map(drop)
}

pub(super) async fn release_unassigned(id: Uuid, maximum_generation: u64) -> Result<()> {
    if record::read(id)?.is_none() {
        return Ok(());
    }
    let generation = {
        let _lock = record::lock(id)?;
        let state = record::required(id)?;
        if state.principal.is_some() || state.generation > maximum_generation {
            return Err(Error::Refused(
                "The source workspace has conflicting ownership.",
            ));
        }
        state.generation
    };
    release(id, generation, true).await.map(drop)
}
