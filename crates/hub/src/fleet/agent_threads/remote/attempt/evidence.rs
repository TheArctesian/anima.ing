use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use super::super::{
    Actor, Error, Evidence, EvidenceOutcome, Launch, LaunchReceipt, Owner, audit, store,
};
use super::original;

pub(in crate::fleet) async fn evidence(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    request: Evidence,
    actor: Actor,
) -> Result<LaunchReceipt, Error> {
    let row = original(conn, owner, request.claim).await?;
    let outcome = match (request.outcome, request.thread, request.cause.as_deref()) {
        (EvidenceOutcome::Created, Some(thread), None) if !thread.is_nil() => {
            require_runner(&row, thread)?;
            "created"
        }
        (
            EvidenceOutcome::NotCreated,
            None,
            Some(cause @ ("refused" | "failed" | "stopped" | "expired")),
        ) if request.publication.is_none() => cause,
        _ => return Err(Error::malformed()),
    };
    write(conn, (owner, &row), outcome, request.publication.is_some()).await?;
    let receipt = reconcile(conn, owner, row.id, at).await?;
    if row.evidence.is_none() || (request.publication.is_some() && !row.withdrawn) {
        result_audit(conn, (owner, at), actor, &receipt).await?;
    }
    Ok(receipt)
}

pub(super) async fn result_audit(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    actor: Actor,
    receipt: &LaunchReceipt,
) -> Result<(), Error> {
    audit::write(
        conn,
        (owner, at),
        actor,
        (
            crate::fleet::events::AuditAction::Update,
            json!({"launch": receipt.launch, "thread": receipt.thread, "actor": "daemon",
               "action": "launch_result", "outcome": receipt.state, "cause": receipt.cause}),
        ),
    )
    .await
}

pub(super) fn require_runner(row: &Launch, thread: Uuid) -> Result<(), Error> {
    if row.authorization.is_none() || row.runner_id != Some(thread) {
        return Err(Error::Refused("LAUNCH_CONFLICT"));
    }
    Ok(())
}

pub(super) async fn write(
    conn: &mut PgConnection,
    (owner, row): (Owner, &Launch),
    outcome: &str,
    withdrawn: bool,
) -> Result<(), Error> {
    if row
        .evidence
        .as_deref()
        .is_some_and(|prior| prior != outcome)
    {
        return Err(Error::Refused("LAUNCH_CONFLICT"));
    }
    sqlx::query(
        "UPDATE agent_threads.launches SET evidence = $3, withdrawn = withdrawn OR $4
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(row.id)
    .bind(outcome)
    .bind(withdrawn)
    .execute(&mut *conn)
    .await?;
    if withdrawn {
        sqlx::query(
            "UPDATE agent_threads.control_offers SET offer = NULL
             WHERE account_id = $1 AND launch_id = $2",
        )
        .bind(owner.account)
        .bind(row.id)
        .execute(&mut *conn)
        .await?;
        close_waiting(conn, (owner, row.id), "sharing_withdrawn").await?;
    } else if outcome != "created" {
        store::close_first(conn, (owner, row, row.refusal_cause(outcome))).await?;
    }
    Ok(())
}

async fn close_waiting(
    conn: &mut PgConnection,
    (owner, launch): (Owner, Uuid),
    cause: &str,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE agent_threads.remote_commands SET state = 'refused', cause = $3, payload = NULL, closed_at = clock_timestamp()
         WHERE account_id = $1 AND launch_id = $2 AND state = 'waiting'",
    )
    .bind(owner.account)
    .bind(launch)
    .bind(cause)
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn never_claimed(
    conn: &mut PgConnection,
    (owner, row): (Owner, &Launch),
    cause: &str,
) -> Result<(), Error> {
    let outcome = if cause == "target_unavailable" {
        "refused"
    } else {
        "expired"
    };
    sqlx::query(
        "UPDATE agent_threads.launches SET evidence = $3, admission_cause = $4
         WHERE account_id = $1 AND launch_id = $2 AND claimed_at IS NULL",
    )
    .bind(owner.account)
    .bind(row.id)
    .bind(outcome)
    .bind(cause)
    .execute(&mut *conn)
    .await?;
    store::close_first(conn, (owner, row, cause)).await?;
    Ok(())
}

pub(in crate::fleet::agent_threads) async fn reconcile(
    conn: &mut PgConnection,
    owner: Owner,
    id: Uuid,
    at: DateTime<Utc>,
) -> Result<LaunchReceipt, Error> {
    let row = store::launch(conn, owner.account, id).await?;
    if row.claimed_at.is_none() && row.evidence.is_none() && at >= row.deadline {
        never_claimed(conn, (owner, &row), "expired").await?;
    } else if row.device_removed_at.is_none()
        && row.authorization.is_some()
        && row.publication.as_deref() != Some("timed_out")
    {
        join_summary(conn, (owner, &row), at).await?;
    }
    store::expire_waiting(conn, owner.device, at).await?;
    Ok(store::launch(conn, owner.account, id).await?.receipt(at))
}

async fn join_summary(
    conn: &mut PgConnection,
    (owner, row): (Owner, &Launch),
    at: DateTime<Utc>,
) -> Result<(), Error> {
    let summary: Option<(Uuid, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, first_published_at FROM agent_threads.summaries
         WHERE device_id = $1 AND runner_id = $2",
    )
    .bind(owner.device)
    .bind(row.runner_id)
    .fetch_optional(&mut *conn)
    .await?;
    let timely_id = summary
        .filter(|(_, stamp)| stamp.is_some_and(|stamp| stamp < row.deadline))
        .map(|(id, _)| id);
    let verdict = if timely_id.is_some() {
        "timely"
    } else if at >= row.deadline {
        "timed_out"
    } else {
        return Ok(());
    };
    let thread = timely_id.filter(|_| row.evidence.as_deref() == Some("created") && !row.withdrawn);
    sqlx::query(
        "UPDATE agent_threads.launches SET publication = COALESCE(publication, $3),
            thread_id = COALESCE(thread_id, $4)
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(row.id)
    .bind(verdict)
    .bind(thread)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet::agent_threads) async fn published(
    conn: &mut PgConnection,
    device: Uuid,
    runner: Uuid,
) -> Result<(), sqlx::Error> {
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT account_id, launch_id FROM agent_threads.launches
         WHERE device_id = $1 AND runner_id = $2 AND \"authorization\" IS NOT NULL",
    )
    .bind(device)
    .bind(runner)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((account, launch)) = row {
        let at = store::now(conn).await?;
        reconcile(conn, Owner { account, device }, launch, at)
            .await
            .map_err(|error| match error {
                Error::Database(error) => error,
                _ => sqlx::Error::Protocol("remote thread publication failed".to_owned()),
            })?;
    }
    Ok(())
}
