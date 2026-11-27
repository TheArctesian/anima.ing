use super::{Error, Result, private_checkout};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::{Action, Command};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(in crate::fleet::agent_threads) async fn enqueue(
    tx: &mut PgConnection,
    device: DeviceId,
    command: &Command,
    now: DateTime<Utc>,
) -> Result<bool> {
    if !command.fits() {
        return Err(Error::Refused("Invalid workspace lifecycle request."));
    }
    let payload = serde_json::to_value(command)?;
    if let Some((held_device, held)) = existing(tx, command.request).await? {
        return identical(device, &payload, held_device, &held).map(|()| false);
    }
    let ready: Option<(bool, bool)> = sqlx::query_as("SELECT ready AND seen_at >= $2,private_checkout FROM agent_threads.workspace_nodes WHERE device_id=$1 FOR UPDATE")
        .bind(device).bind(now-TimeDelta::seconds(30)).fetch_optional(&mut *tx).await?;
    let Some((true, capable)) = ready else {
        return Err(Error::Refused(
            "This node's workspace owner is not connected.",
        ));
    };
    if super::super::handoffs::reserved(&mut *tx, command.workspace).await? {
        return Err(Error::Refused("This workspace has an active handoff."));
    }
    owner(tx, device, command, now).await?;
    let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM agent_threads.workspace_requests WHERE device_id=$1 AND workspace=$2 AND state IN ('queued','offered') AND expires_at>$3)")
        .bind(device).bind(command.workspace).bind(now).fetch_one(&mut *tx).await?;
    if pending {
        return Err(Error::Refused(
            "Wait for this workspace's pending lifecycle request.",
        ));
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM agent_threads.workspace_requests WHERE device_id=$1",
    )
    .bind(device)
    .fetch_one(&mut *tx)
    .await?;
    let limit = if matches!(
        command.action,
        Action::Stop { .. } | Action::Reconcile { .. } | Action::Delete { .. }
    ) {
        4096
    } else {
        4032
    };
    if count >= limit {
        return Err(Error::Refused(
            "This node's workspace request archive is full.",
        ));
    }
    let binding = private_checkout::for_command(tx, device, command, capable).await?;
    let inserted = sqlx::query("INSERT INTO agent_threads.workspace_requests (request,device_id,workspace,principal,payload,state,created_at,expires_at,checkout_binding) VALUES ($1,$2,$3,$4,$5,'queued',$6,$7,$8) ON CONFLICT (request) DO NOTHING")
        .bind(command.request).bind(device).bind(command.workspace).bind(command.principal).bind(&payload)
        .bind(now).bind(now+TimeDelta::minutes(5)).bind(binding.map(sqlx::types::Json)).execute(&mut *tx).await?.rows_affected();
    if inserted == 0 {
        let (held_device, held) = existing(tx, command.request)
            .await?
            .ok_or(Error::Refused("Workspace request disappeared."))?;
        identical(device, &payload, held_device, &held)?;
    }
    Ok(inserted == 1)
}

async fn existing(tx: &mut PgConnection, request: Uuid) -> Result<Option<(DeviceId, Value)>> {
    Ok(sqlx::query_as(
        "SELECT device_id,payload FROM agent_threads.workspace_requests WHERE request=$1",
    )
    .bind(request)
    .fetch_optional(tx)
    .await?)
}
fn identical(device: DeviceId, payload: &Value, held_device: DeviceId, held: &Value) -> Result<()> {
    if held_device == device && held == payload {
        Ok(())
    } else {
        Err(Error::Refused(
            "The request identity belongs to another operation.",
        ))
    }
}
async fn owner(
    tx: &mut PgConnection,
    device: DeviceId,
    command: &Command,
    now: DateTime<Utc>,
) -> Result<()> {
    let owner: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT principal,deleted FROM agent_threads.workspace_owners WHERE device_id=$1 AND workspace=$2",
    )
    .bind(device)
    .bind(command.workspace)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((owner, deleted)) = owner {
        if deleted && !matches!(command.action, Action::Delete { .. }) {
            return Err(Error::Refused(
                "This workspace has been deleted. Use a new workspace identity.",
            ));
        }
        return if owner == command.principal {
            Ok(())
        } else {
            Err(Error::Refused("This workspace belongs to another person."))
        };
    }
    if !matches!(command.action, Action::Prepare { .. }) {
        return Err(Error::Refused("Prepare this workspace first."));
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM agent_threads.workspace_owners WHERE device_id=$1 AND NOT deleted",
    )
    .bind(device)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 128 {
        return Err(Error::Refused("This node's workspace inventory is full."));
    }
    sqlx::query("INSERT INTO agent_threads.workspace_owners(device_id,workspace,principal,created_at) VALUES($1,$2,$3,$4)")
        .bind(device).bind(command.workspace).bind(command.principal).bind(now).execute(tx).await?;
    Ok(())
}

pub(super) async fn status(pool: &PgPool, request: Uuid, principal: Uuid) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('request',request,'device',device_id,'workspace',workspace,'state',state,'receipt',receipt,'expiresAt',expires_at) FROM agent_threads.workspace_requests WHERE request=$1 AND principal=$2")
        .bind(request).bind(principal).fetch_optional(pool).await?)
}

pub(super) async fn inventory(
    pool: &PgPool,
    device: DeviceId,
    principal: Uuid,
    now: DateTime<Utc>,
) -> Result<Value> {
    let observed: Option<(bool, Option<Value>, bool)> = sqlx::query_as(
        "SELECT ready AND seen_at >= $2, capacity, private_checkout FROM agent_threads.workspace_nodes WHERE device_id=$1",
    )
    .bind(device)
    .bind(now - TimeDelta::seconds(30))
    .fetch_optional(pool)
    .await?;
    let (available, capacity, private_checkout) = match observed {
        Some((true, capacity, capable)) => (true, capacity, capable),
        _ => (false, None, false),
    };
    let workspaces: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('workspace',w.workspace,'request',r.request,'state',r.state,'action',r.payload->'action','receipt',r.receipt) FROM agent_threads.workspace_owners w LEFT JOIN LATERAL (SELECT request,state,payload,receipt FROM agent_threads.workspace_requests WHERE device_id=w.device_id AND workspace=w.workspace ORDER BY created_at DESC,request DESC LIMIT 1) r ON true WHERE w.device_id=$1 AND w.principal=$2 AND NOT w.deleted ORDER BY w.created_at,w.workspace LIMIT 128")
        .bind(device).bind(principal).fetch_all(pool).await?;
    Ok(
        json!({"device":device,"available":available,"capacity":capacity,"privateCheckout":private_checkout,"workspaces":workspaces}),
    )
}
