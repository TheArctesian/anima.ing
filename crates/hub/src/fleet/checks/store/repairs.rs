//! Method repairs (CHK-013, R-064). The writes run in the caller's transaction, after
//! [`lock_unchanged`].

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

use super::rows::{config_of, number, status_of, stored_config};
use crate::{
    fleet::checks::{Error, config::CheckConfig, evaluation::CheckSettings},
    ids::{CheckId, DeviceId},
    status::Status,
};

#[derive(Clone, Debug, PartialEq)]
pub(in crate::fleet::checks) struct Facts {
    pub(in crate::fleet::checks) config: Option<CheckConfig>,
    pub(in crate::fleet::checks) settings: CheckSettings,
    pub(in crate::fleet::checks) status: Status,
    pub(in crate::fleet::checks) failures: u32,
    pub(in crate::fleet::checks) paused: bool,
    pub(in crate::fleet::checks) operator_edited: bool,
    pub(in crate::fleet::checks) tried_at: Option<DateTime<Utc>>,
    /// Ever Up or Degraded (D-3); `None` when a stored column already rules out a repair.
    pub(in crate::fleet::checks) answered: Option<bool>,
}

/// A check as the repair read it, so the write can refuse a check that changed since.
pub(in crate::fleet::checks) struct Seen {
    pub(in crate::fleet::checks) facts: Facts,
    pub(in crate::fleet::checks) name: String,
    /// The `config` column as read, compared as stored, not as parsed.
    stored: Value,
}

#[derive(sqlx::FromRow)]
struct FactsRow {
    #[sqlx(rename = "type")]
    kind: String,
    config: Value,
    name: String,
    status: String,
    failures: i32,
    interval_seconds: i32,
    retries: i32,
    retry_interval_seconds: i32,
    degraded_threshold_ms: i32,
    invert: bool,
    paused: bool,
    operator_edited: bool,
    repair_tried_at: Option<DateTime<Utc>>,
    answered: Option<bool>,
}

/// CASE reads the history only when the cheap columns still allow a repair, so a ruled-out
/// check costs one row read per Down beat; the heartbeats come last, as the widest scan.
const FACTS_SQL: &str = "SELECT type, config, name, status, failures, interval_seconds, retries, \
        retry_interval_seconds, degraded_threshold_ms, invert, paused, operator_edited, \
        repair_tried_at, \
        CASE WHEN type = 'icmp' AND NOT operator_edited AND repair_tried_at IS NULL \
                  AND NOT paused AND NOT invert \
        THEN (status IN ('up', 'degraded') \
         OR EXISTS (SELECT 1 FROM checks.uptime_minute u \
                    WHERE u.check_id = c.id AND (u.up_count > 0 OR u.degraded_count > 0)) \
         OR EXISTS (SELECT 1 FROM checks.uptime_hour u \
                    WHERE u.check_id = c.id AND (u.up_count > 0 OR u.degraded_count > 0)) \
         OR EXISTS (SELECT 1 FROM checks.uptime_day u \
                    WHERE u.check_id = c.id AND (u.up_count > 0 OR u.degraded_count > 0)) \
         OR EXISTS (SELECT 1 FROM checks.heartbeats h \
                    WHERE h.check_id = c.id AND h.status IN ('up', 'degraded'))) \
        END AS answered \
     FROM checks.checks c WHERE c.id = $1";

pub(in crate::fleet::checks) async fn facts(
    db: &PgPool,
    id: CheckId,
) -> Result<Option<Seen>, Error> {
    let row: Option<FactsRow> = sqlx::query_as(FACTS_SQL)
        .bind(id)
        .fetch_optional(db)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let facts = Facts {
        config: config_of(&row.kind, row.config.clone()),
        settings: CheckSettings {
            interval_seconds: number(row.interval_seconds)?,
            retries: number(row.retries)?,
            retry_interval_seconds: number(row.retry_interval_seconds)?,
            degraded_threshold_ms: number(row.degraded_threshold_ms)?,
            invert: row.invert,
        },
        status: status_of(&row.status)?,
        failures: number(row.failures)?,
        paused: row.paused,
        operator_edited: row.operator_edited,
        tried_at: row.repair_tried_at,
        answered: row.answered,
    };
    Ok(Some(Seen {
        facts,
        name: row.name,
        stored: row.config,
    }))
}

/// A change committed meanwhile, such as an operator's edit, makes it false (D-8).
pub(in crate::fleet::checks) async fn lock_unchanged(
    conn: &mut PgConnection,
    id: CheckId,
    seen: &Seen,
) -> Result<bool, Error> {
    let locked: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM checks.checks \
         WHERE id = $1 AND type = 'icmp' AND config = $2 AND name = $3 \
           AND NOT operator_edited AND repair_tried_at IS NULL AND NOT paused \
           AND status = 'down' \
         FOR UPDATE",
    )
    .bind(id)
    .bind(&seen.stored)
    .bind(&seen.name)
    .fetch_optional(conn)
    .await?;
    Ok(locked.is_some())
}

pub(in crate::fleet::checks) async fn names_beside(
    conn: &mut PgConnection,
    device: DeviceId,
    id: CheckId,
) -> Result<Vec<String>, Error> {
    Ok(
        sqlx::query_scalar("SELECT name FROM checks.checks WHERE device_id = $1 AND id <> $2")
            .bind(device)
            .bind(id)
            .fetch_all(conn)
            .await?,
    )
}

pub(in crate::fleet::checks) async fn switch_method(
    conn: &mut PgConnection,
    id: CheckId,
    (name, config): (&str, &CheckConfig),
    at: DateTime<Utc>,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE checks.checks SET type = $2, config = $3, name = $4, status = 'unknown', \
                failures = 0, next_due_at = $5, repair_tried_at = $5, updated_at = now() \
         WHERE id = $1",
    )
    .bind(id)
    .bind(config.kind().as_sql())
    .bind(stored_config(config)?)
    .bind(name)
    .bind(at)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet::checks) async fn mark_tried(
    conn: &mut PgConnection,
    id: CheckId,
    at: DateTime<Utc>,
) -> Result<(), Error> {
    sqlx::query("UPDATE checks.checks SET repair_tried_at = $2 WHERE id = $1")
        .bind(id)
        .bind(at)
        .execute(conn)
        .await?;
    Ok(())
}
