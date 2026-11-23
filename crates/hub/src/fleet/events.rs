//! The event log and the audit log (LOG-004, LOG-005) and their live broadcast (LOG-010).

mod actor;
mod bus;
mod chain;
pub mod graphql;
mod model;
mod page;
pub mod process_log;
mod redaction;
mod retention;
mod store;
#[cfg(test)]
mod tests;
mod trail;
mod upkeep;

use chrono::{DateTime, Days, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};

pub use actor::{Actor, ActorKind, TokenKind};
pub use bus::{Bus, Signal};
pub use chain::{check_chain_daily, link_existing};
pub use model::{
    AuditAction, AuditTarget, ClientAddr, EventSummary, HeartbeatSummary, NewAudit, NewEvent,
    Operator, Severity, Source,
};
pub use trail::{Trail, TrailEntry};
pub use upkeep::run_daily;

use crate::ids::EventId;

/// A week of hub downtime or failed runs still finds each day's partition in place (D-4).
const PARTITION_DAYS_AHEAD: u32 = 7;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The message stays generic: the source error may hold SQL text (API-010).
    #[error("the event store is unavailable")]
    Database(#[from] sqlx::Error),
}

impl Error {
    /// A fixed word for the process log, never the source text (LOG-007, API-010).
    #[must_use]
    pub fn fault(&self) -> &'static str {
        let Self::Database(error) = self;
        let code = error
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code);
        if code.as_deref() == Some(LOCK_NOT_AVAILABLE) {
            "lock_timeout"
        } else {
            "database"
        }
    }
}

/// SQLSTATE `lock_not_available`, which `lock_timeout` raises.
const LOCK_NOT_AVAILABLE: &str = "55P03";

/// The daily partition upkeep (D-4, DB-004, DB-005, D-25). Each change waits at most 2 seconds
/// for a lock, so a long reader or writer delays the job, never the reverse; [`run_daily`] retries.
///
/// # Errors
/// `Error::Database` if a partition change fails or a lock is not granted in time.
pub async fn keep_partitions(
    db: &PgPool,
    now: DateTime<Utc>,
    events_keep: Days,
) -> Result<(), Error> {
    create_partitions_ahead(db, now, PARTITION_DAYS_AHEAD).await?;
    drop_expired_partitions(db, now, events_keep).await?;
    Ok(())
}

/// The start check failed (R-016, R-023, LOG-006); it never reaches GraphQL (API-010).
#[derive(Debug, thiserror::Error)]
pub enum RoleError {
    /// The message stays generic: the source error may hold SQL text (API-010).
    #[error("The hub could not check its database login.")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Refused(#[from] Refusal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("The events schema is missing. Run the migrate subcommand before serving.")]
    NotMigrated,
    #[error("The hub's database login is a superuser. Serve as a member of anima_runtime instead.")]
    Superuser,
    #[error(
        "The hub's database login runs as another role. Remove the role setting from the login \
         and from its database URL."
    )]
    SessionRole,
    #[error(
        "The hub's database login owns the database, the events schema or a log table. Run \
         migrations as the owner role and serve as a member of anima_runtime."
    )]
    Owner,
    #[error(
        "The hub's database login can read or write server files or run server programs. \
         Revoke pg_execute_server_program, pg_write_server_files, pg_read_server_files and \
         pg_maintain from it."
    )]
    ServerAccess,
    #[error(
        "The hub's database login can change, lock or reference log entries. Revoke its \
         UPDATE, DELETE, TRUNCATE, TRIGGER, REFERENCES and MAINTAIN rights on the logs and \
         CREATE on the events schema."
    )]
    CanChangeLogs,
    #[error(
        "The hub's database login would get write rights on new tables through default \
         privileges. Remove those default privileges."
    )]
    DefaultPrivileges,
}

/// Checks that the hub's database login cannot change the logs, by itself or through any role
/// it may become (R-016, R-023, LOG-006). Call at start, before serving.
///
/// # Errors
/// `RoleError::Refused` with a plain message (API-010); `RoleError::Database` if it cannot run.
pub async fn check_runtime_role(db: &PgPool) -> Result<(), RoleError> {
    match store::start_refusal(db).await? {
        Some(refusal) => Err(refusal.into()),
        None => Ok(()),
    }
}

/// Returns the names of the dropped partitions (DB-005). The database refuses to drop anything
/// inside retention, by `now` and by its own clock (LOG-006).
///
/// # Errors
/// `Error::Database` if a partition cannot be dropped or the database refuses the cutoff.
pub async fn drop_expired_partitions(
    db: &PgPool,
    now: DateTime<Utc>,
    events_keep: Days,
) -> Result<Vec<String>, Error> {
    let mut dropped = Vec::new();
    for log in retention::LOGS {
        let first_kept_day = log.first_kept_day(now, events_keep);
        dropped.extend(store::drop_day_partitions_before(db, log, first_kept_day, now).await?);
    }
    Ok(dropped)
}

/// Call at start and daily: the fallback in `record` and `audit` holds an `ACCESS EXCLUSIVE` lock
/// until the caller's transaction ends, which blocks readers and can deadlock writers (D-4).
///
/// # Errors
/// `Error::Database` if a partition cannot be created.
pub async fn create_partitions_ahead(
    db: &PgPool,
    today: DateTime<Utc>,
    days: u32,
) -> Result<(), Error> {
    for offset in 0..=days {
        store::create_day_partitions(db, today + TimeDelta::days(i64::from(offset))).await?;
    }
    Ok(())
}

/// `message` never holds a password, key, token or session content (LOG-007); as a backstop,
/// it and the device name are redacted before they are stored and broadcast (D-3).
///
/// The publish happens inside the caller's transaction, so a rollback leaves subscribers with an
/// event that is not stored; the stored event list is the truth (D-5).
///
/// # Errors
/// `Error::Database` if the partition or the row cannot be written.
pub async fn record(
    conn: &mut PgConnection,
    bus: &Bus,
    at: DateTime<Utc>,
    event: NewEvent,
) -> Result<EventId, Error> {
    let event = NewEvent {
        message: redaction::redact_text(&event.message),
        device: event
            .device
            .map(|(id, name)| (id, redaction::redact_text(&name))),
        ..event
    };
    let summary = store::insert_event(conn, at, event).await?;
    let id = summary.id;
    bus.publish(Signal::Event(summary));
    Ok(id)
}

/// Up to `limit` stored events, newest first: those at or after `from` that come before `before`
/// in that order, from `sources` when it is given (TML-007).
///
/// # Errors
/// `Error::Database` if the read fails.
pub async fn before(
    db: &PgPool,
    (from, before): (DateTime<Utc>, (DateTime<Utc>, uuid::Uuid)),
    sources: Option<Vec<Source>>,
    limit: i64,
) -> Result<Vec<EventSummary>, Error> {
    let filter = model::EventFilter {
        sources,
        since: Some(from),
        ..model::EventFilter::default()
    };
    let position = page::Position {
        at: before.0,
        id: before.1,
    };
    Ok(store::list_events(db, &filter, Some(position), limit).await?)
}

/// The stored events in `span`, exclusive of its start, oldest first, at most `limit`.
///
/// # Errors
/// `Error::Database` if the read fails.
pub async fn after(
    conn: &mut PgConnection,
    span: (DateTime<Utc>, DateTime<Utc>),
    limit: i64,
) -> Result<Vec<EventSummary>, Error> {
    Ok(store::events_after(conn, span, limit).await?)
}

/// `text` redacted as `record` stores a message (LOG-007), for text stored outside the logs
/// (BLD-049).
#[must_use]
pub fn redact_text(text: &str) -> String {
    redaction::redact_text(text)
}

/// Secret values in `change` are redacted first as a backstop; callers pass no secrets (LOG-007).
/// The chain lock stays until the caller's transaction ends (LOG-021, LOG-022, LOG-024).
///
/// # Errors
/// `Error::Database` if the partition, the row or the link cannot be written.
pub async fn audit(
    conn: &mut PgConnection,
    at: DateTime<Utc>,
    entry: NewAudit,
) -> Result<(), Error> {
    let entry = NewAudit {
        change: redaction::redact(entry.change),
        ..entry
    };
    let stored = store::insert_audit(conn, at, &entry).await?;
    chain::append(conn, &stored).await?;
    Ok(())
}
