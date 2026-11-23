//! The outbox: notifications decided and not yet finished (D-16).

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgExecutor};

use super::rows::{OUTBOX_COLUMNS, OutboxRow};
use crate::fleet::notifications::{
    Error,
    model::{NewOutbox, Outbox},
};

pub(in crate::fleet::notifications) async fn insert_outbox(
    conn: &mut PgConnection,
    row: &NewOutbox,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO notifications.outbox \
         (id, channel_id, check_id, kind, payload, created_at, next_attempt_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
    )
    .bind(row.id)
    .bind(row.channel)
    .bind(row.check)
    .bind(row.kind.as_str())
    .bind(&row.payload)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

/// The lease end is the claim's token (`Outbox::lease_end`); the claim counts the try, so a lost outcome still runs out of tries.
pub(in crate::fleet::notifications) async fn claim_due(
    db: impl PgExecutor<'_>,
    now: DateTime<Utc>,
    lease: TimeDelta,
    limit: i64,
) -> Result<Vec<Outbox>, Error> {
    let rows: Vec<OutboxRow> = sqlx::query_as(&format!(
        "UPDATE notifications.outbox \
         SET next_attempt_at = $1 + make_interval(secs => $2), attempts = attempts + 1 \
         WHERE id IN ( \
             SELECT id FROM notifications.outbox \
             WHERE sent_at IS NULL AND given_up_at IS NULL AND next_attempt_at <= $1 \
             ORDER BY next_attempt_at, id LIMIT $3 FOR UPDATE SKIP LOCKED) \
         RETURNING {OUTBOX_COLUMNS}"
    ))
    .bind(now)
    .bind(lease.as_seconds_f64())
    .bind(limit)
    .fetch_all(db)
    .await?;
    let mut claimed = rows
        .into_iter()
        .map(Outbox::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    claimed.sort_by_key(|row| row.created_at);
    Ok(claimed)
}

// A claimed row is finished only under its own claim: each statement below matches its lease end (D-16).
// Every claim moves `next_attempt_at` past the one before it, so no two claims share a token.

pub(in crate::fleet::notifications) async fn mark_sent(
    db: impl PgExecutor<'_>,
    claim: &Outbox,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    let done = sqlx::query(
        "UPDATE notifications.outbox SET sent_at = $3 \
         WHERE id = $1 AND next_attempt_at = $2 AND sent_at IS NULL AND given_up_at IS NULL",
    )
    .bind(claim.id)
    .bind(claim.lease_end)
    .bind(at)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub(in crate::fleet::notifications) async fn reschedule(
    db: impl PgExecutor<'_>,
    claim: &Outbox,
    next_attempt_at: DateTime<Utc>,
) -> Result<bool, Error> {
    let done = sqlx::query(
        "UPDATE notifications.outbox SET next_attempt_at = $3 \
         WHERE id = $1 AND next_attempt_at = $2 AND sent_at IS NULL AND given_up_at IS NULL",
    )
    .bind(claim.id)
    .bind(claim.lease_end)
    .bind(next_attempt_at)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub(in crate::fleet::notifications) async fn give_up(
    db: impl PgExecutor<'_>,
    claim: &Outbox,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    let done = sqlx::query(
        "UPDATE notifications.outbox SET given_up_at = $3 \
         WHERE id = $1 AND next_attempt_at = $2 AND sent_at IS NULL AND given_up_at IS NULL",
    )
    .bind(claim.id)
    .bind(claim.lease_end)
    .bind(at)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub(in crate::fleet::notifications) async fn delete_finished_before(
    db: impl PgExecutor<'_>,
    cutoff: DateTime<Utc>,
) -> Result<u64, Error> {
    let done = sqlx::query(
        "DELETE FROM notifications.outbox \
         WHERE coalesce(sent_at, given_up_at) < $1",
    )
    .bind(cutoff)
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}
