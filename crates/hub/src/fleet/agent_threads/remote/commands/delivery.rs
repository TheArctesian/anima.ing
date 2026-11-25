use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use uuid::Uuid;

use super::super::{Actor, Error, Owner, attempt, audit, control, store};
use super::{Payload, offered};

#[derive(Serialize)]
pub(in crate::fleet) struct Delivered {
    id: String,
    account: Uuid,
    device: Uuid,
    launch: Uuid,
    thread: Uuid,
    payload: Payload,
}

type InQueue = (String, Uuid, serde_json::Value, String);

pub(in crate::fleet) async fn poll(
    conn: &mut PgConnection,
    owner: Owner,
    at: DateTime<Utc>,
) -> Result<Vec<Delivered>, Error> {
    store::expire_waiting(conn, owner.device, at).await?;
    let rows: Vec<InQueue> = sqlx::query_as(
        "SELECT command_id, launch_id, payload, state FROM agent_threads.commands
         WHERE account_id = $1 AND device_id = $2 AND state IN ('waiting','delivered')
         ORDER BY CASE kind WHEN 'stop' THEN 0 WHEN 'answer' THEN 1 ELSE 2 END, created_at, command_id",
    )
    .bind(owner.account)
    .bind(owner.device)
    .fetch_all(&mut *conn)
    .await?;
    for (id, launch, body, state) in rows {
        let Some(thread) = release(conn, (owner, at), (launch, &id, &state), &body).await? else {
            continue;
        };
        let payload = serde_json::from_value(body)?;
        if state == "waiting" && !mark_delivery(conn, owner, &id).await? {
            continue;
        }
        return Ok(vec![Delivered {
            id,
            account: owner.account,
            device: owner.device,
            launch,
            thread,
            payload,
        }]);
    }
    Ok(Vec::new())
}

async fn mark_delivery(conn: &mut PgConnection, owner: Owner, id: &str) -> Result<bool, Error> {
    let changed = sqlx::query(
        "WITH sampled AS MATERIALIZED (SELECT clock_timestamp() AS at)
         UPDATE agent_threads.commands SET state = 'delivered', delivered_at = sampled.at
         FROM sampled WHERE account_id = $1 AND command_id = $2
           AND state = 'waiting' AND deadline > sampled.at",
    )
    .bind(owner.account)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(changed.rows_affected() == 1)
}

async fn release(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    (launch, id, state): (Uuid, &str, &str),
    body: &serde_json::Value,
) -> Result<Option<Uuid>, Error> {
    let row = store::launch(conn, owner.account, launch).await?;
    let payload: Payload = serde_json::from_value(body.clone())?;
    if state == "delivered" {
        if matches!(payload, Payload::Prompt { .. })
            && (row.withdrawn
                || row.control_closed
                || store::blocked(conn, owner.device, at).await?
                || !prompt_on(conn, owner, at).await?)
        {
            return Ok(None);
        }
        return Ok(row.runner_id);
    }
    let receipt = attempt::reconcile(conn, owner, launch, at).await?;
    if receipt.binding != "bound" {
        return Ok(None);
    }
    let Some(offer) = control::live(conn, owner, launch, at).await? else {
        return Ok(None);
    };
    if !offered(&offer, &payload) {
        let cause = match payload {
            Payload::Stop { .. } => Some("target_stale"),
            Payload::Answer { .. } => Some("request_expired"),
            Payload::Prompt { .. } => None,
        };
        if let Some(cause) = cause {
            refuse_waiting(conn, (owner, at), (id, cause)).await?;
        }
        return Ok(None);
    }
    if matches!(payload, Payload::Prompt { .. }) && !prompt_allowed(conn, owner, launch, at).await?
    {
        return Ok(None);
    }
    Ok(row.runner_id)
}

async fn refuse_waiting(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    (id, cause): (&str, &str),
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE agent_threads.commands SET state = 'refused', cause = $3, payload = NULL, closed_at = $4
         WHERE account_id = $1 AND command_id = $2 AND state = 'waiting'",
    )
    .bind(owner.account)
    .bind(id)
    .bind(cause)
    .bind(at)
    .execute(conn)
    .await?;
    Ok(())
}

async fn prompt_allowed(
    conn: &mut PgConnection,
    owner: Owner,
    launch: Uuid,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    if store::blocked(conn, owner.device, at).await? {
        return Ok(false);
    }
    if !prompt_on(conn, owner, at).await? {
        return Ok(false);
    }
    let has_stop: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM agent_threads.commands WHERE account_id = $1 AND launch_id = $2
                       AND kind = 'stop' AND state IN ('waiting','delivered'))",
    )
    .bind(owner.account)
    .bind(launch)
    .fetch_one(conn)
    .await?;
    Ok(!has_stop)
}

pub(super) async fn prompt_on(
    conn: &mut PgConnection,
    owner: Owner,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM agent_threads.start_offers
                        WHERE device_id = $1 AND account_id = $2 AND expires_at > $3 AND enabled)",
    )
    .bind(owner.device)
    .bind(owner.account)
    .bind(at)
    .fetch_one(conn)
    .await
    .map_err(Into::into)
}

pub(super) async fn cancel_prompts(
    conn: &mut PgConnection,
    owner: Owner,
    launch: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE agent_threads.commands SET state = 'refused', cause = 'cancelled_by_stop', payload = NULL, closed_at = clock_timestamp()
         WHERE account_id = $1 AND launch_id = $2 AND kind = 'prompt' AND state = 'waiting'",
    )
    .bind(owner.account)
    .bind(launch)
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct CommandResult {
    pub id: String,
    outcome: String,
    cause: Option<String>,
}

impl CommandResult {
    fn valid(&self) -> bool {
        matches!(
            (self.outcome.as_str(), self.cause.as_deref()),
            ("recorded", None)
                | (
                    "refused",
                    Some(
                        "thread_unknown"
                            | "thread_ended"
                            | "threads_off"
                            | "folder_not_permitted"
                            | "request_unknown"
                            | "request_resolved"
                            | "request_expired"
                            | "busy"
                            | "access_changed"
                            | "target_stale"
                            | "invalid_answer"
                            | "cancelled_by_stop",
                    ),
                )
        )
    }
}

pub(in crate::fleet) async fn result(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    result: CommandResult,
    actor: Actor,
) -> Result<(), Error> {
    if !result.valid() {
        return Err(Error::malformed());
    }
    let row: Option<(Uuid, String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT launch_id, state, payload FROM agent_threads.commands
         WHERE account_id = $1 AND device_id = $2 AND command_id = $3 AND delivered_at IS NOT NULL",
    )
    .bind(owner.account)
    .bind(owner.device)
    .bind(&result.id)
    .fetch_optional(&mut *conn)
    .await?;
    let (launch, state, payload) = row.ok_or(Error::Refused("COMMAND_UNKNOWN"))?;
    if state != "delivered" {
        return Ok(());
    }
    let ended = payload
        .and_then(|value| value.pointer("/target/kind").cloned())
        .is_some_and(|kind| kind == "end");
    if ended && result.outcome == "recorded" {
        close_control(conn, owner, launch).await?;
    }
    sqlx::query(
        "UPDATE agent_threads.commands SET state = $3, cause = $4, payload = NULL, closed_at = $5
         WHERE account_id = $1 AND command_id = $2",
    )
    .bind(owner.account)
    .bind(&result.id)
    .bind(&result.outcome)
    .bind(&result.cause)
    .bind(at)
    .execute(&mut *conn)
    .await?;
    audit::write(
        conn,
        (owner, at),
        actor,
        (
            crate::fleet::events::AuditAction::Update,
            serde_json::json!({"launch": launch, "actor": "daemon",
            "action": "command_result", "outcome": result.outcome, "cause": result.cause}),
        ),
    )
    .await
}

async fn close_control(conn: &mut PgConnection, owner: Owner, launch: Uuid) -> Result<(), Error> {
    sqlx::query(
        "UPDATE agent_threads.launches SET control_closed = true WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(launch)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE agent_threads.commands SET state = 'refused', cause = 'thread_ended', payload = NULL, closed_at = clock_timestamp()
         WHERE account_id = $1 AND launch_id = $2 AND state = 'waiting'",
    )
    .bind(owner.account)
    .bind(launch)
    .execute(conn)
    .await?;
    Ok(())
}
