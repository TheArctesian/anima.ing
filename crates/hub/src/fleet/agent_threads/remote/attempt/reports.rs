use chrono::{DateTime, Utc};
use daemon_core::wire::WorkState;
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

use super::super::{Actor, Error, Launch, Owner};
use super::{LEASE, evidence, live_lease};

#[derive(Deserialize)]
struct End {
    outcome: String,
    cause: Option<String>,
    seconds: u64,
    cleanup: daemon_core::wire::Cleanup,
    thread: Option<Uuid>,
}

#[derive(Deserialize)]
struct ThreadReport {
    id: Uuid,
    attempt: u32,
    state: WorkState,
    result: Option<End>,
}

pub(in crate::fleet) async fn report(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    body: &serde_json::Value,
    actor: Actor,
) -> Result<Option<serde_json::Value>, Error> {
    let request: ThreadReport =
        serde_json::from_value(body.clone()).map_err(|_| Error::malformed())?;
    let row: Option<Launch> = sqlx::query_as(
        "SELECT * FROM agent_threads.launches WHERE account_id = $1 AND work_id = $2",
    )
    .bind(owner.account)
    .bind(request.id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else { return Ok(None) };
    if row.device_id != owner.device
        || row.device_removed_at.is_some()
        || row
            .evidence
            .as_deref()
            .is_some_and(|code| code != "created")
        || request.attempt != 1
        || live_lease(&row, at).is_none()
    {
        return Err(Error::Refused("NOT_HOLDER"));
    }
    if !allowed(row.attempt_phase.as_deref(), request.state) {
        return Err(Error::malformed());
    }
    if let Some(result) = request.result.as_ref() {
        settle(conn, (owner, &row), request.state, result).await?;
    } else if request.state.is_final() {
        return Err(Error::malformed());
    }
    let lease_end = at + LEASE;
    let lease = (!request.state.is_final()).then_some(lease_end);
    sqlx::query(
        "UPDATE agent_threads.launches SET attempt_phase = $3, lease_end = $4
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(row.id)
    .bind(request.state.as_str())
    .bind(lease)
    .execute(&mut *conn)
    .await?;
    if request.state.is_final() {
        let receipt = evidence::reconcile(conn, owner, row.id, at).await?;
        if row.evidence.is_none() {
            evidence::result_audit(conn, (owner, at), actor, &receipt).await?;
        }
    }
    Ok(Some(
        serde_json::json!({ "lease": {"until": lease.unwrap_or(at)}, "logEnd": 0 }),
    ))
}

fn allowed(from: Option<&str>, to: WorkState) -> bool {
    use WorkState::{Claimed, Done, Failed, Finishing, Preparing, Refused, Running, Stopped};
    match from {
        Some("claimed") => matches!(to, Claimed | Preparing | Refused | Failed | Stopped),
        Some("preparing") => matches!(to, Preparing | Running | Refused | Failed | Stopped),
        Some("running") => matches!(to, Running | Finishing | Failed | Stopped),
        Some("finishing") => matches!(to, Finishing | Done | Failed | Stopped),
        _ => false,
    }
}

async fn settle(
    conn: &mut PgConnection,
    (owner, row): (Owner, &Launch),
    state: WorkState,
    result: &End,
) -> Result<(), Error> {
    if result.seconds > 120
        || [
            &result.cleanup.isolation,
            &result.cleanup.checkout,
            &result.cleanup.secrets,
            &result.cleanup.keychain,
        ]
        .into_iter()
        .any(|value| value != "none")
    {
        return Err(Error::malformed());
    }
    let phase = state.as_str();
    match (phase, result.outcome.as_str(), result.thread) {
        ("done", "succeeded", Some(thread)) if !thread.is_nil() && result.cause.is_none() => {
            evidence::require_runner(row, thread)?;
            evidence::write(conn, (owner, row), "created", false).await
        }
        ("refused", "refused", None)
            if matches!(
                result.cause.as_deref(),
                Some("threads_off" | "program_not_permitted" | "folder_not_permitted")
            ) =>
        {
            if row.authorization.is_none() {
                evidence::write(conn, (owner, row), "refused", false).await?;
            }
            Ok(())
        }
        ("failed", "failed", None) | ("stopped", "stopped", None) => Ok(()),
        _ => Err(Error::malformed()),
    }
}
