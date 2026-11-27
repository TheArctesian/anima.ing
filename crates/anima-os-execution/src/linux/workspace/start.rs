//! Start one reserved workspace generation under hosting-first admission.
mod dispatch;

use super::{
    Error, Result, checkout,
    record::{self, Phase, Record},
    runtime,
};
use anima_os_maintenance::{
    reservations::{self, Reservation},
    source_pipeline::Pipeline,
};
use daemon_core::source_jobs::Grant;
use dispatch::{LockedWorkspace, SourceAuthority};
use serde_json::Value;
use uuid::Uuid;

pub async fn start(id: Uuid, reservation: Reservation) -> Result<Value> {
    start_with(id, reservation, None, None).await
}

pub(in crate::linux) async fn start_managed(
    id: Uuid,
    reservation: Reservation,
    principal: Uuid,
    credential: Option<checkout::ManagedCredential>,
) -> Result<Value> {
    start_with(id, reservation, None, Some((principal, credential))).await
}

pub(in crate::linux) async fn start_source(
    pipeline: Pipeline,
    reservation: Reservation,
    grant: Grant,
) -> Result<Value> {
    if (reservation.run, reservation.generation) != (pipeline.execution(), 1) {
        return Err(Error::Refused(
            "The workspace reservation does not match the source execution.",
        ));
    }
    start_with(
        pipeline.workspace(),
        reservation,
        Some(SourceAuthority { pipeline, grant }),
        None,
    )
    .await
}

async fn start_with(
    id: Uuid,
    reservation: Reservation,
    source: Option<SourceAuthority>,
    managed: Option<(Uuid, Option<checkout::ManagedCredential>)>,
) -> Result<Value> {
    if reservation.runtime != reservations::RuntimeOwner::Docker {
        return Err(Error::Refused(
            "Workspace execution requires a Docker reservation.",
        ));
    }
    let locks = LockedWorkspace::acquire(id, source)?;
    let mut state = record::required(id)?;
    if let Some(source) = source {
        if state.principal.is_some() {
            return Err(Error::Refused(
                "The checkout workspace belongs to another owner.",
            ));
        }
        source.authorize()?;
    }
    let credential = if let Some((principal, credential)) = managed {
        if state.principal != Some(principal) {
            return Err(Error::Refused("Workspace belongs to another owner."));
        }
        credential
    } else {
        None
    };
    state.require_launchable()?;
    if state.generation == reservation.generation && state.run == Some(reservation.run) {
        if state.resources != Some(reservation.resources) {
            return Err(Error::Refused(
                "The existing run has different resource limits.",
            ));
        }
        reservations::require(&locks.admission, &reservation)?;
        return runtime::observe(&state).await;
    }
    checkout::allow_launch(&state, credential.as_ref())?;
    record::check_run_owner(id, reservation.run)?;
    if !matches!(state.phase, Phase::Prepared | Phase::Stopped)
        || reservation.generation
            != state
                .generation
                .checked_add(1)
                .ok_or(Error::Refused("Workspace generation exhausted."))?
    {
        return Err(Error::Refused(
            "Refresh the workspace state before starting a new generation.",
        ));
    }
    reservations::reserve(&locks.admission, reservation.clone())?;
    state.generation = reservation.generation;
    state.run = Some(reservation.run);
    state.resources = Some(reservation.resources);
    state.phase = Phase::Starting;
    record::save(&state)?;
    launch(state, locks, &reservation, credential).await
}

async fn launch(
    mut state: Record,
    locks: LockedWorkspace,
    reservation: &Reservation,
    credential: Option<checkout::ManagedCredential>,
) -> Result<Value> {
    let _workspace = locks.dispatch(&mut state, reservation).await?;
    let result = checkout::finish_planned(&mut state, credential).await;
    state.phase = if result.is_ok() {
        Phase::Running
    } else {
        Phase::Unknown
    };
    record::save(&state)?;
    result?;
    runtime::observe(&state).await
}
