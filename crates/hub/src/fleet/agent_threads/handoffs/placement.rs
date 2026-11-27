//! An authorized move waits for a destination before the source is fenced.
mod queue;

use super::{Error, MoveRequest, Offer, Result, Start, begin, source::Limits, store};
use crate::ids::DeviceId;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, types::Json};
use uuid::Uuid;

pub(super) use queue::{lock_identity, lock_request, queue, withdraw};

pub(super) async fn request_named(
    tx: &mut PgConnection,
    principal: Uuid,
    start: Start,
    now: DateTime<Utc>,
) -> Result<bool> {
    lock_request(tx, start.request).await?;
    if start.request.continue_session {
        return queue(
            tx,
            principal,
            (start.request, Some(start.destination_device)),
            now,
        )
        .await;
    }
    queue::require_named(tx, start.request, now).await?;
    begin::start(tx, principal, start, now).await
}

pub(super) async fn status(
    db: &PgPool,
    transfer: Uuid,
    principal: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Value>> {
    let found = store::status(db, transfer, principal, now).await?;
    if found.is_some() {
        return Ok(found);
    }
    queue::status(db, transfer, principal, now).await
}

#[derive(sqlx::FromRow)]
struct Candidate {
    transfer: Uuid,
    workspace: Uuid,
    principal: Uuid,
    source_device: DeviceId,
    destination: DeviceId,
    generation: i64,
    storage_bytes: i64,
    continue_session: bool,
    limits: Json<Limits>,
}

impl Candidate {
    fn start(&self) -> Result<Start> {
        let generation =
            u64::try_from(self.generation).map_err(|_| Error::Refused("Invalid generation."))?;
        let storage_bytes = u64::try_from(self.storage_bytes)
            .map_err(|_| Error::Refused("Invalid storage quota."))?;
        Ok(Start {
            request: MoveRequest {
                transfer: self.transfer,
                workspace: self.workspace,
                source_device: self.source_device,
                generation,
                storage_bytes,
                continue_session: self.continue_session,
            },
            destination_device: self.destination,
            headroom: Some(self.limits.0),
        })
    }
}

pub(super) async fn issue(
    tx: &mut PgConnection,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Option<Offer>> {
    queue::expire(tx, device, now).await?;
    let Some(candidate) = next(tx, device, now).await? else {
        return Ok(None);
    };
    let placed = place(tx, &candidate, now).await?;
    let state = if placed { "placed" } else { "waiting" };
    sqlx::query(
        "UPDATE agent_threads.workspace_move_queue SET state=$2,checked_at=$3 WHERE transfer=$1",
    )
    .bind(candidate.transfer)
    .bind(state)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    if !placed {
        return Ok(None);
    }
    let transfer = store::find(tx, candidate.transfer)
        .await?
        .ok_or(Error::Refused(
            "The placed workspace transfer is unavailable.",
        ))?;
    store::export_offer(transfer).map(Some)
}

async fn place(tx: &mut PgConnection, candidate: &Candidate, now: DateTime<Utc>) -> Result<bool> {
    // The poll already holds the source device. A reciprocal poll may hold the destination.
    let destination = sqlx::query_scalar::<_, DeviceId>(
        "SELECT id FROM devices.devices WHERE id=$1 FOR KEY SHARE SKIP LOCKED",
    )
    .bind(candidate.destination)
    .fetch_optional(&mut *tx)
    .await?;
    if destination.is_none() {
        return Ok(false);
    }
    match begin::start(tx, candidate.principal, candidate.start()?, now).await {
        Ok(_) => Ok(true),
        Err(Error::Refused(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn next(
    tx: &mut PgConnection,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Option<Candidate>> {
    Ok(sqlx::query_as("SELECT q.*, destination.device_id AS destination
        FROM agent_threads.workspace_move_queue q
        JOIN agent_threads.workspace_nodes source ON source.device_id=q.source_device
        JOIN LATERAL (
            SELECT n.device_id FROM agent_threads.workspace_nodes n
            JOIN devices.devices d ON d.id=n.device_id
            WHERE n.ready AND n.seen_at >= $2 AND n.device_id<>q.source_device AND d.metrics_source='daemon' AND NOT d.maintenance
              AND (q.destination_device IS NULL OR q.destination_device=n.device_id)
              AND agent_threads.workspace_placement_available(n.device_id,$1)
              AND agent_threads.workspace_destination_available(n.device_id,q.workspace,q.principal,q.generation)
              AND (n.capacity->>'storageBytes')::bigint >= q.storage_bytes
              AND (n.capacity->>'memoryBytes')::bigint >= (q.limits->>'memoryBytes')::bigint
              AND (n.capacity->>'cpuMillis')::bigint >= (q.limits->>'cpuMillis')::bigint
              AND (n.capacity->>'tasks')::bigint >= (q.limits->>'tasks')::bigint
            ORDER BY (n.capacity->>'memoryBytes')::bigint DESC,
                     (n.capacity->>'cpuMillis')::bigint DESC,n.device_id
            LIMIT 1
        ) destination ON true
        WHERE q.source_device=$3 AND q.state='waiting' AND q.expires_at>$1 AND source.ready AND source.seen_at >= $2
          AND NOT EXISTS (SELECT 1 FROM agent_threads.workspace_handoffs active
                          WHERE active.workspace=q.workspace AND active.finished_at IS NULL)
          AND NOT EXISTS (SELECT 1 FROM agent_threads.workspace_requests active WHERE active.device_id=q.source_device
                          AND active.workspace=q.workspace AND active.state IN ('queued','offered','unresolved'))
        ORDER BY q.checked_at NULLS FIRST,q.created_at,q.transfer LIMIT 1 FOR UPDATE OF q SKIP LOCKED")
        .bind(now).bind(now - TimeDelta::seconds(30)).bind(device).fetch_optional(tx).await?)
}
