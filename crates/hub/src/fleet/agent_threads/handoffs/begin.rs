use super::{Error, Result, Start, Transfer, source};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::{Capacity, Phase};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

pub(super) async fn start(
    tx: &mut PgConnection,
    principal: Uuid,
    start: Start,
    now: DateTime<Utc>,
) -> Result<bool> {
    if principal.is_nil() || !start.fits() {
        return Err(Error::Refused("Invalid workspace transfer request."));
    }
    let request = start.request;
    if let Some(held) = super::store::find(tx, request.transfer).await? {
        return identical(&held, principal, start).map(|()| false);
    }
    require_nodes(tx, start, now).await?;
    let source = source::current(tx, principal, request).await?;
    if start.headroom.is_some() && source.storage_bytes != request.storage_bytes {
        return Err(Error::Refused(
            "The source workspace storage quota has changed.",
        ));
    }
    if destination_conflict(tx, principal, start).await? {
        return Err(Error::Refused(
            "Resolve the pending workspace operation or choose a different destination.",
        ));
    }
    let expires = now + TimeDelta::hours(1);
    let inserted = sqlx::query("INSERT INTO agent_threads.workspace_handoffs
        (transfer,workspace,principal,source_device,destination_device,generation,storage_bytes,checkout,created_at,expires_at,continue_session)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT DO NOTHING")
        .bind(request.transfer).bind(request.workspace).bind(principal)
        .bind(request.source_device).bind(start.destination_device)
        .bind(i64::try_from(request.generation).map_err(|_| Error::Refused("Invalid generation."))?)
        .bind(i64::try_from(request.storage_bytes).map_err(|_| Error::Refused("Invalid storage quota."))?)
        .bind(source.checkout.map(|checkout| Json(checkout.source)))
        .bind(now).bind(expires).bind(request.continue_session).execute(&mut *tx).await?.rows_affected();
    if inserted == 0 {
        let held = super::store::find(tx, request.transfer)
            .await?
            .ok_or(Error::Refused("This workspace already has a transfer."))?;
        identical(&held, principal, start)?;
        return Ok(false);
    }
    if request.continue_session && matches!(source.phase, Phase::Running) {
        super::continuation::capture(tx, request, principal).await?;
    }
    super::super::managed::close_for_handoff(tx, request.source_device, request.workspace).await?;
    Ok(true)
}

pub(super) fn identical(held: &Transfer, principal: Uuid, start: Start) -> Result<()> {
    let request = start.request;
    if held.principal != principal
        || held.continue_session != request.continue_session
        || held.workspace != request.workspace
        || held.source_device != request.source_device
        || held.destination_device != start.destination_device
        || u64::try_from(held.generation) != Ok(request.generation)
        || u64::try_from(held.storage_bytes) != Ok(request.storage_bytes)
    {
        return Err(Error::Refused(
            "The transfer identity already belongs to another request.",
        ));
    }
    Ok(())
}

async fn require_nodes(tx: &mut PgConnection, start: Start, now: DateTime<Utc>) -> Result<()> {
    let request = start.request;
    // Device removal and daemon lifecycle polls both take the device row first.
    let enrolled: Vec<DeviceId> = sqlx::query_scalar(
        "SELECT id FROM devices.devices WHERE id IN ($1,$2) AND metrics_source='daemon' AND (NOT $3 OR id=$1 OR NOT maintenance)
         ORDER BY id FOR KEY SHARE",
    )
    .bind(request.source_device)
    .bind(start.destination_device)
    .bind(start.headroom.is_some())
    .fetch_all(&mut *tx)
    .await?;
    if enrolled.len() != 2 {
        return Err(Error::Refused(
            "Both nodes must be enrolled for workspace transfer.",
        ));
    }
    let rows: Vec<(DeviceId, Option<Json<Capacity>>)> = sqlx::query_as(
        "SELECT device_id,capacity FROM agent_threads.workspace_nodes
         WHERE device_id IN ($1,$2) AND ready AND seen_at >= $3
         ORDER BY device_id FOR UPDATE",
    )
    .bind(request.source_device)
    .bind(start.destination_device)
    .bind(now - TimeDelta::seconds(30))
    .fetch_all(&mut *tx)
    .await?;
    let destination_ready = rows.iter().any(|(id, capacity)| {
        *id == start.destination_device
            && capacity.as_ref().is_some_and(|capacity| {
                capacity.0.storage_bytes >= request.storage_bytes
                    && start.headroom.is_none_or(|limits| limits.fit(capacity.0))
            })
    });
    if rows.len() != 2 || !destination_ready {
        return Err(Error::Refused(
            "Both nodes must be connected and the destination must report sufficient storage.",
        ));
    }
    if start.headroom.is_some() {
        let allowed: bool =
            sqlx::query_scalar("SELECT agent_threads.workspace_placement_available($1,$2)")
                .bind(start.destination_device)
                .bind(now)
                .fetch_one(tx)
                .await?;
        if !allowed {
            return Err(Error::Refused(
                "This node is unavailable for workspace placement.",
            ));
        }
    }
    Ok(())
}

async fn destination_conflict(
    tx: &mut PgConnection,
    principal: Uuid,
    start: Start,
) -> Result<bool> {
    let request = start.request;
    let generation =
        i64::try_from(request.generation).map_err(|_| Error::Refused("Invalid generation."))?;
    Ok(sqlx::query_scalar(
        "SELECT NOT agent_threads.workspace_destination_available($1,$3,$4,$5) OR EXISTS (
        SELECT 1 FROM agent_threads.workspace_requests
        WHERE device_id=$2 AND workspace=$3 AND state IN ('queued','offered','unresolved')
    )",
    )
    .bind(start.destination_device)
    .bind(request.source_device)
    .bind(request.workspace)
    .bind(principal)
    .bind(generation)
    .fetch_one(tx)
    .await?)
}
