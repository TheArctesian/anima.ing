use super::super::{Error, MoveRequest, Result, Start, begin, source, store};
use crate::ids::DeviceId;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, types::Json};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Held {
    workspace: Uuid,
    principal: Uuid,
    source_device: DeviceId,
    destination_device: Option<DeviceId>,
    generation: i64,
    storage_bytes: i64,
    continue_session: bool,
    state: String,
}

pub(in crate::fleet::agent_threads::handoffs) async fn lock_identity(
    tx: &mut PgConnection,
    transfer: Uuid,
) -> Result<()> {
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('workspace move:' || $1::uuid::text, 0))",
    )
    .bind(transfer)
    .execute(tx)
    .await?;
    Ok(())
}

pub(in crate::fleet::agent_threads::handoffs) async fn lock_request(
    tx: &mut PgConnection,
    request: MoveRequest,
) -> Result<()> {
    lock_identity(tx, request.transfer).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('workspace move source:' || $1::uuid::text, 0))")
        .bind(request.workspace).execute(tx).await?;
    Ok(())
}

pub(in crate::fleet::agent_threads::handoffs) async fn require_named(
    tx: &mut PgConnection,
    request: MoveRequest,
    now: DateTime<Utc>,
) -> Result<()> {
    let conflict: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_threads.workspace_move_queue
        WHERE transfer=$1 OR (workspace=$2 AND state='waiting' AND expires_at>$3
          AND NOT EXISTS(SELECT 1 FROM agent_threads.workspace_handoffs replay WHERE replay.transfer=$1)))",
    )
    .bind(request.transfer)
    .bind(request.workspace)
    .bind(now)
    .fetch_one(tx)
    .await?;
    if conflict {
        return Err(Error::Refused("This workspace already has a move request."));
    }
    Ok(())
}

pub(in crate::fleet::agent_threads::handoffs) async fn queue(
    tx: &mut PgConnection,
    principal: Uuid,
    (request, destination): (MoveRequest, Option<DeviceId>),
    now: DateTime<Utc>,
) -> Result<bool> {
    let named = destination.map(|destination_device| Start {
        request,
        destination_device,
        headroom: None,
    });
    if principal.is_nil() || !request.fits() || named.is_some_and(|start| !start.fits()) {
        return Err(Error::Refused("Invalid workspace transfer request."));
    }
    if let Some(held) = held(tx, request.transfer).await? {
        return replay(&held, principal, request, destination);
    }
    if let Some(transfer) = store::find(tx, request.transfer).await? {
        return named
            .ok_or(Error::Refused("This workspace already has a transfer."))
            .and_then(|start| begin::identical(&transfer, principal, start))
            .map(|()| false);
    }
    if super::super::reserved(tx, request.workspace).await? {
        return Err(Error::Refused("This workspace already has a transfer."));
    }
    let view = source::current(tx, principal, request).await?;
    if view.storage_bytes != request.storage_bytes {
        return Err(Error::Refused(
            "The source workspace storage quota has changed.",
        ));
    }
    let limits = source::limits(tx, principal, request)
        .await?
        .ok_or(Error::Refused(
            "The source workspace has no recorded runtime limits.",
        ))?;
    if let Some(destination) = destination {
        require_enrolled_destination(tx, destination).await?;
    }
    persist(tx, principal, (request, destination, limits), now).await?;
    Ok(true)
}

async fn require_enrolled_destination(tx: &mut PgConnection, destination: DeviceId) -> Result<()> {
    let enrolled = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM devices.devices WHERE id=$1 AND metrics_source='daemon')",
    )
    .bind(destination)
    .fetch_one(tx)
    .await?;
    if !enrolled {
        return Err(Error::Refused("Choose an enrolled destination node."));
    }
    Ok(())
}

async fn persist(
    tx: &mut PgConnection,
    principal: Uuid,
    (request, destination, limits): (MoveRequest, Option<DeviceId>, source::Limits),
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_threads.workspace_move_queue SET state='expired'
        WHERE workspace=$1 AND state='waiting' AND expires_at<=$2",
    )
    .bind(request.workspace)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let inserted = sqlx::query("INSERT INTO agent_threads.workspace_move_queue
        (transfer,workspace,principal,source_device,generation,storage_bytes,continue_session,limits,created_at,expires_at,destination_device)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT DO NOTHING")
        .bind(request.transfer).bind(request.workspace).bind(principal).bind(request.source_device)
        .bind(i64::try_from(request.generation).map_err(|_| Error::Refused("Invalid generation."))?)
        .bind(i64::try_from(request.storage_bytes).map_err(|_| Error::Refused("Invalid storage quota."))?)
        .bind(request.continue_session).bind(Json(limits)).bind(now).bind(now+TimeDelta::hours(1)).bind(destination)
        .execute(tx).await?;
    if inserted.rows_affected() == 0 {
        return Err(Error::Refused("This workspace already has a move request."));
    }
    Ok(())
}

pub(super) async fn expire(
    tx: &mut PgConnection,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query("UPDATE agent_threads.workspace_move_queue SET state='expired' WHERE transfer IN (
        SELECT transfer FROM agent_threads.workspace_move_queue WHERE source_device=$1 AND state='waiting' AND expires_at<=$2
        ORDER BY expires_at LIMIT 16 FOR UPDATE SKIP LOCKED
    )")
    .bind(device).bind(now).execute(tx).await?;
    Ok(())
}

async fn held(tx: &mut PgConnection, transfer: Uuid) -> Result<Option<Held>> {
    Ok(sqlx::query_as(
        "SELECT workspace,principal,source_device,destination_device,generation,storage_bytes,continue_session,state
        FROM agent_threads.workspace_move_queue WHERE transfer=$1 FOR UPDATE",
    )
    .bind(transfer)
    .fetch_optional(tx)
    .await?)
}

fn replay(
    held: &Held,
    principal: Uuid,
    request: MoveRequest,
    destination: Option<DeviceId>,
) -> Result<bool> {
    if held.workspace != request.workspace
        || held.principal != principal
        || held.source_device != request.source_device
        || held.destination_device != destination
        || u64::try_from(held.generation) != Ok(request.generation)
        || u64::try_from(held.storage_bytes) != Ok(request.storage_bytes)
        || held.continue_session != request.continue_session
    {
        return Err(Error::Refused(
            "The transfer identity already belongs to another request.",
        ));
    }
    Ok(false)
}

pub(in crate::fleet::agent_threads::handoffs) async fn withdraw(
    tx: &mut PgConnection,
    (principal, device): (Uuid, DeviceId),
    (transfer, workspace): (Uuid, Uuid),
) -> Result<Option<bool>> {
    let Some(held) = held(tx, transfer).await? else {
        return Ok(None);
    };
    if held.principal != principal || held.source_device != device || held.workspace != workspace {
        return Err(Error::Refused("This workspace transfer is unavailable."));
    }
    if held.state == "placed" {
        return Ok(None);
    }
    if held.state != "waiting" {
        return Ok(Some(false));
    }
    sqlx::query(
        "UPDATE agent_threads.workspace_move_queue SET state='withdrawn' WHERE transfer=$1",
    )
    .bind(transfer)
    .execute(tx)
    .await?;
    Ok(Some(true))
}

pub(in crate::fleet::agent_threads::handoffs) async fn status(
    db: &PgPool,
    transfer: Uuid,
    principal: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('transfer',transfer,'workspace',workspace,
           'sourceDevice',source_device,'destinationDevice',destination_device,'generation',generation,'expiresAt',expires_at,
           'state',CASE WHEN state='withdrawn' THEN 'withdrawn'
                        WHEN state='expired' OR expires_at<=$3 THEN 'placement-expired'
                        ELSE 'waiting-placement' END)
        FROM agent_threads.workspace_move_queue WHERE transfer=$1 AND principal=$2 AND state<>'placed'")
        .bind(transfer).bind(principal).bind(now).fetch_optional(db).await?)
}
