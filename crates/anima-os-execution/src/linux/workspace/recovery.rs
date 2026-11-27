use super::{
    Error, Result,
    record::{self, Phase, Record},
    runtime,
};
use crate::linux::command;
use anima_os_maintenance::{
    Admission,
    reservations::{self, Reservation, RuntimeOwner},
};
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

/// Preparation has never launched work; finish the recorded storage intent only.
pub(super) async fn finish_preparing(mut state: Record) -> Result<Value> {
    if state.phase != Phase::Preparing
        || state.generation != 0
        || state.run.is_some()
        || state.incoming.is_some()
    {
        return Err(Error::Refused(
            "Workspace preparation has conflicting runtime history.",
        ));
    }
    make_home(&state).await?;
    state.phase = Phase::Prepared;
    record::save(&state)?;
    Ok(serde_json::to_value(state)?)
}

pub(super) async fn make_home(state: &Record) -> Result<()> {
    let home = record::home(state.workspace);
    let path = home
        .to_str()
        .ok_or(Error::Refused("Invalid workspace storage path."))?;
    match fs::symlink_metadata(&home) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            command::run("btrfs", &["subvolume", "create", path]).await?;
        }
        Ok(metadata) if metadata.is_dir() => {
            command::run("btrfs", &["subvolume", "show", path]).await?;
        }
        Err(error) => return Err(error.into()),
        _ => {
            return Err(Error::Refused(
                "Workspace storage is not its recorded subvolume.",
            ));
        }
    }
    command::run(
        "btrfs",
        &["qgroup", "limit", &state.storage_bytes.to_string(), path],
    )
    .await?;
    for path in [
        home.clone(),
        home.join("workspace"),
        home.join(".config"),
        home.join(".config/anima-daemon"),
    ] {
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)?,
            Ok(metadata) if metadata.is_dir() => {}
            Err(error) => return Err(error.into()),
            _ => {
                return Err(Error::Refused(
                    "Workspace preparation contains an unexpected path.",
                ));
            }
        }
        nix::unistd::chown(
            &path,
            Some(nix::unistd::Uid::from_raw(1000)),
            Some(nix::unistd::Gid::from_raw(1000)),
        )
        .map_err(std::io::Error::from)?;
    }
    Ok(())
}

pub async fn reconcile(workspace: Uuid, generation: u64) -> Result<Value> {
    let admission = Admission::cleanup()?;
    let _lock = record::lock(workspace)?;
    let state = record::required(workspace)?;
    if state.handoff.is_some() {
        if state.generation != generation {
            return Err(Error::Refused("The workspace generation changed."));
        }
        let state = super::stop_locked(state, &admission).await?;
        return Ok(json!({"workspace":state,"dataRetained":true}));
    }
    if state.principal.is_some()
        && state.generation.checked_add(1) == Some(generation)
        && matches!(state.phase, Phase::Prepared | Phase::Stopped)
    {
        return Ok(serde_json::to_value(state)?);
    }
    if state.generation != generation {
        return Err(Error::Refused("The workspace generation changed."));
    }
    if matches!(
        state.phase,
        Phase::Preparing | Phase::Prepared | Phase::Deleting | Phase::Deleted
    ) {
        return Err(Error::Refused(
            "Retry preparation with the original storage request.",
        ));
    }
    reconcile_runtime(state, &admission).await
}

async fn reconcile_runtime(mut state: Record, admission: &Admission) -> Result<Value> {
    let generation = state.generation;
    let present = command::docker(&[
        "ps",
        "--all",
        "--quiet",
        "--filter",
        &format!("name=^{}$", state.container()),
    ])
    .await?;
    if !present.trim().is_empty() {
        let observed = runtime::observe(&state).await?;
        if observed["runtime"]["running"] == true && state.checkout_ready() {
            if state.phase == Phase::Stopping || state.phase == Phase::Stopped {
                return Err(Error::Refused(
                    "The stop intent is unresolved. Retry stopping this generation.",
                ));
            }
            let reservation = Reservation {
                schema: 1,
                runtime: RuntimeOwner::Docker,
                run: state
                    .run
                    .ok_or(Error::Refused("The running workspace has no run identity."))?,
                generation,
                resources: state.resources.ok_or(Error::Refused(
                    "The running workspace has no resource limits.",
                ))?,
            };
            reservations::require(admission, &reservation)?;
            state.phase = Phase::Running;
            record::save(&state)?;
            return Ok(
                json!({"workspace":state,"runtime":observed["runtime"],"runnerReady":observed["runnerReady"],"replayed":false}),
            );
        }
    }
    // A running container cannot confirm a generation with an incomplete checkout.
    // Release capacity only after the engine proves no run-labelled containers remain.
    state.phase = Phase::Stopping;
    record::save(&state)?;
    runtime::stop(&state).await?;
    if let Some(run) = state.run {
        reservations::release_stopped(admission, run, generation).await?;
    }
    state.phase = Phase::Stopped;
    record::save(&state)?;
    Ok(json!({"workspace":state,"dataRetained":true,"replayed":false}))
}
