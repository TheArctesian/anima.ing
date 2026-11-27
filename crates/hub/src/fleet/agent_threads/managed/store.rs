use super::{Error, Result};
use crate::ids::DeviceId;
use agent_threads::managed::Command;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(in crate::fleet::agent_threads) async fn enqueue(
    tx: &mut PgConnection,
    device: DeviceId,
    command: &Command,
    now: DateTime<Utc>,
) -> Result<bool> {
    if !command.fits() {
        return Err(Error::Refused("Invalid managed Code command."));
    }
    let payload = serde_json::to_value(command)?;
    if let Some((held_device, held)) = sqlx::query_as::<_, (DeviceId, Value)>(
        "SELECT device_id, payload FROM agent_threads.commands WHERE request=$1",
    )
    .bind(command.request)
    .fetch_optional(&mut *tx)
    .await?
    {
        return if held_device == device && held == payload {
            Ok(false)
        } else {
            Err(Error::Refused(
                "The request identity belongs to another command.",
            ))
        };
    }
    ready(tx, device, command, now).await?;
    #[cfg(unix)]
    if matches!(command.action, agent_threads::control::Action::Start { .. })
        && super::super::handoffs::continuation::pending_agent(tx, device, command).await?
    {
        return Err(Error::Refused(
            "Wait for the previous session's continuation to settle.",
        ));
    }
    let inserted = sqlx::query(
        "INSERT INTO agent_threads.commands
        (request,device_id,workspace,generation,principal,payload,state,created_at,expires_at)
        VALUES ($1,$2,$3,$4,$5,$6,'queued',$7,$8) ON CONFLICT (request) DO NOTHING",
    )
    .bind(command.request)
    .bind(device)
    .bind(command.workspace)
    .bind(i64::try_from(command.generation)?)
    .bind(command.principal)
    .bind(&payload)
    .bind(now)
    .bind(now + TimeDelta::minutes(5))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        let (held_device, held): (DeviceId, Value) =
            sqlx::query_as("SELECT device_id,payload FROM agent_threads.commands WHERE request=$1")
                .bind(command.request)
                .fetch_one(&mut *tx)
                .await?;
        if held_device != device || held != payload {
            return Err(Error::Refused(
                "The request identity belongs to another command.",
            ));
        }
    }
    Ok(inserted == 1)
}

pub(super) async fn enabled(
    tx: &mut PgConnection,
    device: DeviceId,
    workspace: Uuid,
    enabled: bool,
) -> Result<()> {
    if enabled {
        // Serialize against the handoff owner's locks on both node rows.
        sqlx::query(
            "SELECT device_id FROM agent_threads.workspace_nodes WHERE device_id=$1 FOR UPDATE",
        )
        .bind(device)
        .execute(&mut *tx)
        .await?;
        if super::super::handoffs::reserved(&mut *tx, workspace).await? {
            return Err(Error::Refused("This workspace has an active handoff."));
        }
    }
    let updated = sqlx::query(
        "UPDATE agent_threads.workspaces SET enabled=$3 WHERE device_id=$1 AND workspace=$2",
    )
    .bind(device)
    .bind(workspace)
    .bind(enabled)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(Error::Refused("The device has not offered this workspace."));
    }
    if !enabled {
        revoke(tx, device, workspace).await?;
    }
    Ok(())
}

#[cfg(unix)]
pub(in crate::fleet::agent_threads) async fn close_for_handoff(
    tx: &mut PgConnection,
    device: DeviceId,
    workspace: Uuid,
) -> std::result::Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE agent_threads.workspaces SET enabled=false WHERE device_id=$1 AND workspace=$2",
    )
    .bind(device)
    .bind(workspace)
    .execute(&mut *tx)
    .await?;
    revoke(tx, device, workspace).await
}

async fn revoke(
    tx: &mut PgConnection,
    device: DeviceId,
    workspace: Uuid,
) -> std::result::Result<(), sqlx::Error> {
    sqlx::query("UPDATE agent_threads.previews SET revoked=true WHERE device_id=$1 AND workspace=$2 AND NOT revoked")
        .bind(device).bind(workspace).execute(&mut *tx).await?;
    sqlx::query("UPDATE agent_threads.commands SET state=CASE WHEN state='queued' THEN 'revoked' ELSE 'unresolved' END
        WHERE device_id=$1 AND workspace=$2 AND state IN ('queued','offered')")
        .bind(device).bind(workspace).execute(tx).await?;
    Ok(())
}

pub(super) async fn status(pool: &PgPool, request: Uuid, principal: Uuid) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object('request',request,'device',device_id,
        'workspace',workspace,'generation',generation,'state',state,'result',result,
        'hubThread',(SELECT s.id FROM agent_threads.summaries s
          WHERE s.device_id=c.device_id AND s.runner_id=c.request))
        FROM agent_threads.commands c WHERE request=$1 AND principal=$2",
    )
    .bind(request)
    .bind(principal)
    .fetch_optional(pool)
    .await?)
}

pub(super) async fn workspaces(
    pool: &PgPool,
    device: DeviceId,
    now: DateTime<Utc>,
    principal: Option<Uuid>,
) -> Result<Vec<Value>> {
    let rows = sqlx::query_scalar(
        "SELECT jsonb_build_object('workspace',workspace,'generation',generation,
        'enabled',enabled,'ready',(ready AND seen_at >= $2),'seenAt',seen_at,
        'listeningPorts',CASE WHEN $3 AND enabled AND ready AND seen_at >= $2
            THEN listening_ports ELSE '{}'::integer[] END)
        FROM agent_threads.workspaces WHERE device_id=$1 ORDER BY workspace LIMIT 128",
    )
    .bind(device)
    .bind(now - TimeDelta::seconds(30))
    .bind(principal.is_some())
    .fetch_all(pool)
    .await?;
    #[cfg(unix)]
    if let Some(principal) = principal {
        return Ok(
            super::super::handoffs::continuation::with_views(pool, device, principal, rows).await?,
        );
    }
    Ok(rows)
}

async fn ready(
    tx: &mut PgConnection,
    device: DeviceId,
    command: &Command,
    now: DateTime<Utc>,
) -> Result<()> {
    let ready = sqlx::query_scalar::<_, bool>(
        "SELECT enabled AND ready AND generation=$3 AND seen_at >= $4
         FROM agent_threads.workspaces WHERE device_id=$1 AND workspace=$2 FOR UPDATE",
    )
    .bind(device)
    .bind(command.workspace)
    .bind(i64::try_from(command.generation)?)
    .bind(now - TimeDelta::seconds(30))
    .fetch_optional(&mut *tx)
    .await?;
    if ready != Some(true) {
        return Err(Error::Refused(
            "Enable a connected, ready workspace before sending a command.",
        ));
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM agent_threads.commands WHERE device_id=$1 AND workspace=$2",
    )
    .bind(device)
    .bind(command.workspace)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 4096 {
        return Err(Error::Refused("This workspace's command archive is full."));
    }
    Ok(())
}
