use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{
    actor::{ActorKind, TokenKind},
    chain::StoredEntry,
    model::{AuditRecord, EventFilter, EventSummary, NewAudit, NewEvent, Severity, Source},
    page::Position,
    retention::{LOGS, Log},
};
use crate::ids::{DeviceId, EventId};

pub(super) mod chain;
mod following;
mod start_check;
pub(super) use following::events_after;
pub(super) use start_check::start_refusal;

const ENSURE_DAY_PARTITION: &str = "select events.ensure_day_partition($1, $2)";

/// Runs in the caller's transaction (D-4).
async fn ensure_day_partition(
    conn: &mut PgConnection,
    parent: &str,
    at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(ENSURE_DAY_PARTITION)
        .bind(parent)
        .bind(at)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Waiting longer would queue every reader and writer of the log; the daily job retries (D-4).
const SCHEDULED_LOCK_TIMEOUT: &str = "set local lock_timeout = '2s'";

/// Never a caller's transaction, so the lock ends at its commit.
async fn scheduled_change(db: &PgPool) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query(SCHEDULED_LOCK_TIMEOUT)
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

pub(super) async fn create_day_partitions(
    db: &PgPool,
    at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    for log in LOGS {
        let mut tx = scheduled_change(db).await?;
        sqlx::query(ENSURE_DAY_PARTITION)
            .bind(log.table())
            .bind(at)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

/// The database refuses a day inside the retention period of `now` or of its own clock.
pub(super) async fn drop_day_partitions_before(
    db: &PgPool,
    log: Log,
    first_kept_day: NaiveDate,
    now: DateTime<Utc>,
) -> Result<Vec<String>, sqlx::Error> {
    let mut tx = scheduled_change(db).await?;
    let dropped = sqlx::query_scalar("select events.drop_day_partitions_before($1, $2, $3)")
        .bind(log.table())
        .bind(first_kept_day)
        .bind(now)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(dropped)
}

/// Returns the event as stored, with `at` at the database's microsecond precision.
pub(super) async fn insert_event(
    conn: &mut PgConnection,
    at: DateTime<Utc>,
    event: NewEvent,
) -> Result<EventSummary, sqlx::Error> {
    ensure_day_partition(conn, "event_log", at).await?;
    let id = EventId::from(Uuid::now_v7());
    let (device_id, device_name) = event
        .device
        .as_ref()
        .map(|(id, name)| (*id, name.as_str()))
        .unzip();
    let at = sqlx::query_scalar(
        "insert into events.event_log \
         (id, at, severity, source, device_id, device_name, message, actor_kind) \
         values ($1, $2, $3, $4, $5, $6, $7, $8) returning at",
    )
    .bind(id)
    .bind(at)
    .bind(event.severity.as_str())
    .bind(event.source.as_str())
    .bind(device_id)
    .bind(device_name)
    .bind(&event.message)
    .bind(event.actor.as_str())
    .fetch_one(&mut *conn)
    .await?;
    Ok(EventSummary {
        id,
        at,
        severity: event.severity,
        source: event.source,
        device: event.device,
        message: event.message,
        actor: Some(event.actor),
    })
}

/// The caller has already redacted `change`; the actor's token is never stored (LOG-018).
pub(super) async fn insert_audit(
    conn: &mut PgConnection,
    at: DateTime<Utc>,
    entry: &NewAudit,
) -> Result<StoredEntry, sqlx::Error> {
    ensure_day_partition(conn, "audit_log", at).await?;
    let actor = &entry.operator.actor;
    let stored: chain::EntryRow = sqlx::query_as(&format!(
        "insert into events.audit_log \
         (id, at, operator_id, operator_name, action, target_type, target_id, client_addr, \
         change, actor_kind, token_kind, api_key_name) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
         returning {}",
        chain::ENTRY_COLUMNS
    ))
    .bind(Uuid::now_v7())
    .bind(at)
    .bind(entry.operator.id)
    .bind(&entry.operator.name)
    .bind(entry.action.as_str())
    .bind(entry.target.kind())
    .bind(entry.target.id())
    .bind(entry.client.0)
    .bind(&entry.change)
    .bind(actor.kind().as_str())
    .bind(actor.token().map(TokenKind::as_str))
    .bind(actor.api_key_name())
    .fetch_one(&mut *conn)
    .await?;
    Ok(stored.into())
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    at: DateTime<Utc>,
    severity: String,
    source: String,
    device_id: Option<DeviceId>,
    device_name: Option<String>,
    message: String,
    actor_kind: Option<String>,
}

impl TryFrom<EventRow> for EventSummary {
    type Error = sqlx::Error;

    fn try_from(row: EventRow) -> Result<Self, Self::Error> {
        let unknown = |column: &str| sqlx::Error::Decode(format!("unknown event {column}").into());
        Ok(Self {
            id: row.id.into(),
            at: row.at,
            severity: Severity::from_stored(&row.severity).ok_or_else(|| unknown("severity"))?,
            source: Source::from_stored(&row.source).ok_or_else(|| unknown("source"))?,
            device: row.device_id.zip(row.device_name),
            message: row.message,
            actor: stored_kind(row.actor_kind.as_deref(), ActorKind::from_stored)?,
        })
    }
}

pub(super) async fn list_events(
    db: &PgPool,
    filter: &EventFilter,
    after: Option<Position>,
    limit: i64,
) -> Result<Vec<EventSummary>, sqlx::Error> {
    let severities: Option<Vec<&str>> = filter
        .severities
        .as_ref()
        .map(|all| all.iter().map(|severity| severity.as_str()).collect());
    let sources: Option<Vec<&str>> = filter
        .sources
        .as_ref()
        .map(|all| all.iter().map(|source| source.as_str()).collect());
    let rows: Vec<EventRow> = sqlx::query_as(
        "select id, at, severity, source, device_id, device_name, message, actor_kind \
         from events.event_log \
         where ($1::text[] is null or severity = any($1)) \
           and ($2::text[] is null or source = any($2)) \
           and ($3::uuid is null or device_id = $3) \
           and ($4::timestamptz is null or at >= $4) \
           and ($5::timestamptz is null or (at, id) < ($5, $6)) \
           and ($8::text is null \
                or strpos(lower(message || ' ' || coalesce(device_name, '')), lower($8)) > 0) \
         order by at desc, id desc limit $7",
    )
    .bind(severities)
    .bind(sources)
    .bind(filter.device)
    .bind(filter.since)
    .bind(after.map(|position| position.at))
    .bind(after.map(|position| position.id))
    .bind(limit)
    .bind(filter.text.as_deref())
    .fetch_all(db)
    .await?;
    rows.into_iter().map(EventSummary::try_from).collect()
}

#[derive(sqlx::FromRow)]
struct AuditRow {
    id: Uuid,
    at: DateTime<Utc>,
    operator_name: String,
    action: String,
    target_type: String,
    target_id: String,
    client_addr: std::net::IpAddr,
    change: serde_json::Value,
    actor_kind: Option<String>,
    token_kind: Option<String>,
    api_key_name: Option<String>,
}

/// `None` for a row written before the column existed; an error for text this hub does not know.
fn stored_kind<T>(
    text: Option<&str>,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, sqlx::Error> {
    text.map(|text| {
        parse(text).ok_or_else(|| sqlx::Error::Decode("unknown actor or token kind".into()))
    })
    .transpose()
}

/// An entry with no recorded kind matches only `None` (LOG-020).
pub(super) async fn list_audit(
    db: &PgPool,
    actor: Option<ActorKind>,
    after: Option<Position>,
    limit: i64,
) -> Result<Vec<AuditRecord>, sqlx::Error> {
    let rows: Vec<AuditRow> = sqlx::query_as(
        "select id, at, operator_name, action, target_type, target_id, client_addr, change, \
         actor_kind, token_kind, api_key_name \
         from events.audit_log \
         where ($1::text is null or actor_kind = $1) \
           and ($2::timestamptz is null or (at, id) < ($2, $3)) \
         order by at desc, id desc limit $4",
    )
    .bind(actor.map(ActorKind::as_str))
    .bind(after.map(|position| position.at))
    .bind(after.map(|position| position.id))
    .bind(limit)
    .fetch_all(db)
    .await?;
    rows.into_iter().map(AuditRecord::try_from).collect()
}

impl TryFrom<AuditRow> for AuditRecord {
    type Error = sqlx::Error;

    fn try_from(row: AuditRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            at: row.at,
            operator_name: row.operator_name,
            action: row.action,
            target_type: row.target_type,
            target_id: row.target_id,
            client: row.client_addr,
            change: row.change,
            actor_kind: stored_kind(row.actor_kind.as_deref(), ActorKind::from_stored)?,
            token_kind: stored_kind(row.token_kind.as_deref(), TokenKind::from_stored)?,
            api_key_name: row.api_key_name,
        })
    }
}
