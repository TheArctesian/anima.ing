//! The use of the AI agent programs on each device (R-135; AIS-018 to AIS-024; K8 Section 4).
//!
//! Model and repository names come from a device: they are checked at ingest and reach SQL only
//! as bound values.

pub mod accounts;
pub mod graphql;
mod grouping;
mod model;
pub mod prices;
pub mod sessions;
mod store;
#[cfg(test)]
mod tests;

use chrono::{DateTime, NaiveDate, Utc};
use daemon_core::agent_usage::AgentUsage;
use sqlx::{PgConnection, PgPool};

pub use model::{Ask, Dimension, DimensionSet, Report, Session, SessionList, StoredSession};

use crate::ids::DeviceId;

const MAX_GRAINS: usize = 20_000;

/// Why an agent usage operation failed. Messages are fixed text: they hold no SQL and no
/// value from a device.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored agent usage row cannot be read")]
    Unreadable,
    #[error("too many groups")]
    TooManyGroups,
}

/// Stores `usage` from a batch of `device` in the caller's transaction (K8 Section 4, R-313):
/// each session day replaces what is stored for that session and day.
///
/// # Errors
/// `Database` when a statement fails; nothing of the batch is then kept.
pub async fn record(
    conn: &mut PgConnection,
    device: DeviceId,
    usage: &AgentUsage,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    if let Some(state) = usage.state {
        store::write_state(conn, device, state).await?;
    }
    store::write_session_days(conn, device, &usage.session_days).await?;
    if let Some(accounts) = &usage.accounts {
        accounts::record(conn, device, accounts, now).await?;
    }
    Ok(())
}

/// The sums `ask` asks for, the total and the usage state of each device; groups are in no set
/// order.
///
/// # Errors
/// `TooManyGroups` past [`MAX_GRAINS`]; `Database` when a query fails; `Unreadable` for a
/// stored source or state this version does not know.
pub async fn usage(db: &PgPool, ask: &Ask) -> Result<Report, Error> {
    let grains = store::grains(db, ask, ask.by, MAX_GRAINS).await?;
    let groups = grouping::fold(grains, ask.by);
    let total_grains = store::grains(db, ask, DimensionSet::default(), MAX_GRAINS).await?;
    let total = grouping::fold(total_grains, DimensionSet::default())
        .pop()
        .unwrap_or_default();
    let states = store::states(db, ask.device).await?;
    Ok(Report {
        groups,
        total,
        states,
    })
}

/// At most `limit` sessions that `ask` admits, the newest use first, and how many it admits in
/// all (AIS-025). `ask.by` is not read.
///
/// # Errors
/// `Database` when a query fails; `Unreadable` for a stored source this version does not know.
pub async fn sessions(db: &PgPool, ask: &Ask, limit: usize) -> Result<SessionList, Error> {
    store::sessions(db, ask, limit).await
}

/// The first and last day of the default 30-day range ending `today` (AIS-020).
#[must_use]
pub fn default_days(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let first = today
        .checked_sub_days(chrono::Days::new(model::DEFAULT_DAYS - 1))
        .unwrap_or(today);
    (first, today)
}
