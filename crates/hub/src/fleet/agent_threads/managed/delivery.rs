use super::{Error, Result};
use crate::ids::DeviceId;
use agent_threads::managed::{
    Command, Delivery, Report,
    servers::{Inventory, ServerPoll},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::PgConnection;
use std::collections::HashMap;
use uuid::Uuid;

/// The ingest edge holds the device lock and rechecks its credential in this transaction.
pub(in crate::fleet) async fn poll(
    tx: &mut PgConnection,
    device: DeviceId,
    request: ServerPoll,
    now: DateTime<Utc>,
) -> Result<Delivery> {
    if !request.fits() || request.poll.device != Uuid::from(device) {
        return Err(Error::Refused("Invalid managed Code poll."));
    }
    let ServerPoll { poll, servers } = request;
    let inventory = Inventory {
        workspaces: poll.workspaces,
        servers,
    };
    offers(tx, device, inventory, now).await?;
    let mut acknowledged = Vec::new();
    for report in poll.reports {
        acknowledged.push(acknowledge(tx, device, report).await?);
    }
    sqlx::query("UPDATE agent_threads.commands SET state=CASE WHEN state='queued' THEN 'expired' ELSE 'unresolved' END
        WHERE device_id=$1 AND expires_at <= $2 AND state IN ('queued','offered')")
        .bind(device).bind(now).execute(&mut *tx).await?;
    #[cfg(unix)]
    super::super::handoffs::continuation::queue_agent(tx, device, now).await?;
    let next: Option<(Uuid, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT c.request,c.payload,c.expires_at FROM agent_threads.commands c
         JOIN agent_threads.workspaces w USING(device_id,workspace)
         WHERE c.device_id=$1 AND c.state IN ('queued','offered') AND c.expires_at>$2
           AND w.enabled AND w.ready AND w.generation=c.generation AND w.seen_at >= $3
         ORDER BY c.created_at,c.request LIMIT 1 FOR UPDATE OF c",
    )
    .bind(device)
    .bind(now)
    .bind(now - TimeDelta::seconds(30))
    .fetch_optional(&mut *tx)
    .await?;
    let (command, valid_until) = if let Some((request, payload, expires)) = next {
        sqlx::query("UPDATE agent_threads.commands SET state='offered' WHERE request=$1")
            .bind(request)
            .execute(tx)
            .await?;
        (
            Some(serde_json::from_value(payload)?),
            Some(expires.min(now + TimeDelta::seconds(15))),
        )
    } else {
        (None, None)
    };
    Ok(Delivery {
        command,
        valid_until,
        acknowledged,
    })
}

async fn offers(
    tx: &mut PgConnection,
    device: DeviceId,
    inventory: Inventory,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_threads.workspaces SET ready=false,listening_ports='{}' WHERE device_id=$1",
    )
    .bind(device)
    .execute(&mut *tx)
    .await?;
    let ports: HashMap<_, _> = inventory
        .servers
        .into_iter()
        .map(|row| {
            (
                row.workspace,
                row.ports.into_iter().map(i32::from).collect::<Vec<_>>(),
            )
        })
        .collect();
    for offer in inventory.workspaces {
        let listening = ports.get(&offer.workspace).map_or(&[][..], Vec::as_slice);
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM agent_threads.workspaces WHERE device_id=$1")
                .bind(device)
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query("INSERT INTO agent_threads.workspaces AS old (device_id,workspace,generation,ready,seen_at,listening_ports)
            SELECT $1,$2,$3,$4,$5,$6 WHERE $7 < 128 OR EXISTS (
                SELECT 1 FROM agent_threads.workspaces WHERE device_id=$1 AND workspace=$2)
            ON CONFLICT (device_id,workspace) DO UPDATE SET
            generation=EXCLUDED.generation,ready=EXCLUDED.ready,seen_at=EXCLUDED.seen_at,listening_ports=EXCLUDED.listening_ports
            WHERE old.generation <= EXCLUDED.generation")
            .bind(device).bind(offer.workspace).bind(i64::try_from(offer.generation)?).bind(offer.ready)
            .bind(now).bind(listening).bind(count).execute(&mut *tx).await?;
    }
    Ok(())
}

async fn acknowledge(tx: &mut PgConnection, device: DeviceId, report: Report) -> Result<Uuid> {
    let held: Option<(Uuid, i64, Uuid, Value, String, Option<Value>)> = sqlx::query_as(
        "SELECT workspace,generation,principal,payload,state,result FROM agent_threads.commands WHERE request=$1 AND device_id=$2 FOR UPDATE")
        .bind(report.request).bind(device).fetch_optional(&mut *tx).await?;
    let Some((workspace, generation, principal, payload, held_state, held_result)) = held else {
        return Err(Error::Refused("Unknown managed Code report."));
    };
    if report.workspace != workspace
        || report.generation != u64::try_from(generation)?
        || report.principal != principal
    {
        return Err(Error::Refused("Managed Code report identity mismatch."));
    }
    let command: Command = serde_json::from_value(payload)?;
    if report
        .result
        .as_ref()
        .is_some_and(|result| !command.accepts(result))
    {
        return Err(Error::Refused(
            "Managed Code result does not match the command.",
        ));
    }
    let result = serde_json::to_value(&report.result)?;
    if held_state == "completed" {
        if held_result != Some(result) {
            return Err(Error::Refused(
                "A completed command receipt cannot be changed.",
            ));
        }
        return Ok(report.request);
    }
    if !matches!(held_state.as_str(), "offered" | "unresolved") {
        return Err(Error::Refused(
            "This command has not been offered to the device.",
        ));
    }
    let state = if report.result.is_some() {
        "completed"
    } else {
        "unresolved"
    };
    // A completed receipt is immutable. Repeated polls only acknowledge it.
    sqlx::query(
        "UPDATE agent_threads.commands SET state=$3,result=$4
        WHERE request=$1 AND device_id=$2 AND state IN ('offered','unresolved')",
    )
    .bind(report.request)
    .bind(device)
    .bind(state)
    .bind(result)
    .execute(&mut *tx)
    .await?;
    Ok(report.request)
}
