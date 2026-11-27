use super::{Error, Result};
use crate::{ids::DeviceId, work::checkout::workspace::Binding};
use agent_threads::managed::workspace::{Action, Capacity, Delivery, Poll, Receipt};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

/// The enrolled-device edge holds the device and credential locks for this transaction.
pub(in crate::fleet) async fn poll(
    tx: &mut PgConnection,
    device: DeviceId,
    (poll, capacity, private_checkout): (Poll, Option<Capacity>, bool),
    now: DateTime<Utc>,
) -> Result<Delivery> {
    if !poll.fits() || poll.device != Uuid::from(device) {
        return Err(Error::Refused("Invalid workspace poll."));
    }
    sqlx::query("INSERT INTO agent_threads.workspace_nodes(device_id,ready,seen_at,capacity,private_checkout) VALUES($1,$2,$3,$4,$5) ON CONFLICT(device_id) DO UPDATE SET ready=EXCLUDED.ready,seen_at=EXCLUDED.seen_at,capacity=EXCLUDED.capacity,private_checkout=EXCLUDED.private_checkout")
        .bind(device).bind(poll.ready).bind(now).bind(capacity.map(Json)).bind(private_checkout).execute(&mut *tx).await?;
    let mut acknowledged = Vec::new();
    if let Some(report) = poll.report {
        acknowledge(tx, device, &report).await?;
        acknowledged.push(report.command.request);
    }
    sqlx::query("UPDATE agent_threads.workspace_requests SET state=CASE WHEN state='queued' THEN 'expired' ELSE 'unresolved' END WHERE device_id=$1 AND expires_at <= $2 AND state IN ('queued','offered')")
        .bind(device).bind(now).execute(&mut *tx).await?;
    #[cfg(unix)]
    if let Some(capacity) = capacity.filter(|_| poll.ready) {
        super::super::handoffs::continuation::queue_workspace(tx, device, capacity, now).await?;
    }
    let next = if poll.ready {
        eligible(
            tx,
            device,
            now,
            (capacity.unwrap_or(Capacity::MAXIMUM), private_checkout),
        )
        .await?
    } else {
        None
    };
    let (command, valid_until, checkout_repository) = if let Some((
        request,
        payload,
        expires,
        binding,
    )) = next
    {
        let command: agent_threads::managed::workspace::Command = serde_json::from_value(payload)?;
        let valid_until = expires.min(now + TimeDelta::seconds(15));
        let repository = binding
            .filter(|_| matches!(command.action, Action::Start { .. }))
            .map(|Json(binding)| Uuid::from(binding.repository));
        sqlx::query("UPDATE agent_threads.workspace_requests SET state='offered',checkout_offer_until=$2 WHERE request=$1")
            .bind(request)
            .bind(repository.map(|_| valid_until))
            .execute(tx)
            .await?;
        (Some(command), Some(valid_until), repository)
    } else {
        (None, None, None)
    };
    Ok(Delivery {
        command,
        valid_until,
        acknowledged,
        checkout_repository,
    })
}

async fn eligible(
    tx: &mut PgConnection,
    device: DeviceId,
    now: DateTime<Utc>,
    (capacity, private_checkout): (Capacity, bool),
) -> Result<Option<(Uuid, Value, DateTime<Utc>, Option<Json<Binding>>)>> {
    // An offered request can carry a durable receipt even if headroom changed.
    Ok(sqlx::query_as("SELECT request,payload,expires_at,checkout_binding FROM agent_threads.workspace_requests
         WHERE device_id=$1 AND state IN ('queued','offered') AND expires_at>$2
         AND ($4 OR checkout_binding IS NULL)
         AND (state='offered' OR CASE payload->'action'->>'kind'
           WHEN 'prepare' THEN (payload->'action'->>'storage_bytes')::bigint <= ($3::jsonb->>'storageBytes')::bigint
           WHEN 'start' THEN (payload->'action'->>'memory_bytes')::bigint <= ($3::jsonb->>'memoryBytes')::bigint
                         AND (payload->'action'->>'cpu_millis')::bigint <= ($3::jsonb->>'cpuMillis')::bigint
                         AND (payload->'action'->>'tasks')::bigint <= ($3::jsonb->>'tasks')::bigint
           ELSE true END)
         ORDER BY created_at,request LIMIT 1 FOR UPDATE")
        .bind(device).bind(now).bind(Json(capacity)).bind(private_checkout).fetch_optional(tx).await?)
}

async fn acknowledge(tx: &mut PgConnection, device: DeviceId, report: &Receipt) -> Result<()> {
    let row: Option<(Value, String, Option<Value>)> = sqlx::query_as("SELECT payload,state,receipt FROM agent_threads.workspace_requests WHERE request=$1 AND device_id=$2 FOR UPDATE")
        .bind(report.command.request).bind(device).fetch_optional(&mut *tx).await?;
    let Some((payload, state, previous)) = row else {
        return Err(Error::Refused("Unknown workspace receipt."));
    };
    if payload != serde_json::to_value(&report.command)? || !report.fits() {
        return Err(Error::Refused("Workspace receipt identity mismatch."));
    }
    let receipt = serde_json::to_value(report)?;
    if state == "completed" {
        return if previous == Some(receipt) {
            Ok(())
        } else {
            Err(Error::Refused(
                "A completed workspace receipt is immutable.",
            ))
        };
    }
    if !matches!(state.as_str(), "offered" | "unresolved") {
        return Err(Error::Refused("This workspace request was not delivered."));
    }
    let state = if report.result.is_some() {
        "completed"
    } else {
        "unresolved"
    };
    sqlx::query("UPDATE agent_threads.workspace_requests SET state=$3,receipt=$4 WHERE request=$1 AND device_id=$2")
        .bind(report.command.request).bind(device).bind(state).bind(receipt).execute(&mut *tx).await?;
    if report.result.is_some() && matches!(report.command.action, Action::Delete { .. }) {
        sqlx::query("UPDATE agent_threads.workspace_owners SET deleted=true WHERE device_id=$1 AND workspace=$2 AND principal=$3")
            .bind(device).bind(report.command.workspace).bind(report.command.principal).execute(tx).await?;
    }
    Ok(())
}
