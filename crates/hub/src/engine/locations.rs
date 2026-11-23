//! Location samples and the context of a transfer event (K-E4 Sections 11 and 12; R-145).
//! A position is sealed with the sealing key `locations` (Class B, VLT-003, R-146). Every way in
//! stores through [`store_samples`], and no code replaces an older sample.

mod context;
mod dawarich;
pub mod graphql;
mod model;
mod owntracks;
mod routes;
mod store;
#[cfg(test)]
mod tests;
mod tokens;
mod trail;

use chrono::{DateTime, DurationRound as _, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub use context::{
    Actor, ActorKind, Basis, Direction, End, MatchRule, OperatorEvidence, RULE,
    Stored as StoredContext, inside, operator_evidence,
};
pub use dawarich::dawarich_intake;
pub use model::{Bad, IngestToken, NewSample, Origin, Position, Provider, Sample, Stored};
pub use routes::router;
pub use tokens::{Issued, admit, create_token, revoke_token, tokens};

use crate::{
    engine::{
        places,
        sealing_keys::{SealingKey, SealingKeyError, SealingKeys},
    },
    fleet::events,
    ids::{DeviceId, EventId, SampleId, SiteId},
};

const KEY_NAME: &str = "locations";
pub const MAX_SPAN: TimeDelta = TimeDelta::days(31);
pub const MAX_READ: i64 = 1000;
pub const MAX_BATCH: usize = 500;
/// Far more than a phone gives in the 20 minutes around an event.
const NEAR_LIMIT: i64 = 5000;

/// Why a locations operation failed. No message holds a position, a token or a key (LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("the audit entry cannot be written ({0})")]
    Trail(#[from] crate::fleet::events::Error),
    #[error("the sealing key of the positions cannot be used: {0}")]
    Key(#[from] SealingKeyError),
    #[error("the places cannot be read")]
    Places(#[from] places::Error),
    #[error("a stored position does not open")]
    Damaged,
    #[error("{} is not valid", .0.iter().map(|bad| bad.field.as_str()).collect::<Vec<_>>().join(", "))]
    Invalid(Vec<Bad>),
    #[error("no such ingest token")]
    NotFound,
}

/// Without a folder of keys every write and every read of a position is `Key(Unavailable)`.
#[derive(Clone)]
pub struct Seal {
    keys: SealingKeys,
}

impl Seal {
    #[must_use]
    pub const fn new(keys: SealingKeys) -> Self {
        Self { keys }
    }

    fn key(&self) -> Result<SealingKey, Error> {
        Ok(self.keys.key(KEY_NAME)?)
    }
}

fn sealing_context(id: SampleId) -> String {
    format!("locations.sample:{id}")
}

/// A sample that the hub has (the same provider, account and key) stays as it is, so the same
/// batch again changes nothing.
///
/// # Errors
/// `Invalid` with each field to fix, and then nothing is stored; `Key`; `Database`.
pub async fn store_samples(
    conn: &mut PgConnection,
    seal: &Seal,
    (origin, samples): (&Origin, &[NewSample]),
    at: DateTime<Utc>,
) -> Result<Stored, Error> {
    let bad: Vec<Bad> = samples
        .iter()
        .enumerate()
        .flat_map(|(index, sample)| model::faults(index, sample))
        .collect();
    if !bad.is_empty() {
        return Err(Error::Invalid(bad));
    }
    if samples.is_empty() {
        return Ok(Stored::default());
    }
    let key = seal.key()?;
    let mut stored = Stored::default();
    for sample in samples {
        let id = SampleId::from(Uuid::now_v7());
        let plain = serde_json::to_vec(&sample.position).map_err(|_| Error::Damaged)?;
        let sealed = key.seal(&sealing_context(id), &plain);
        let times = (sample.observed_at, at);
        if store::insert_sample(conn, (id, origin, &sample.key), times, &sealed).await? {
            stored.added += 1;
        } else {
            stored.known += 1;
        }
    }
    Ok(stored)
}

fn opened(key: &SealingKey, row: store::SampleRow) -> Result<Sample, Error> {
    let plain = key
        .open(&sealing_context(row.id), &row.sealed)
        .map_err(|_| Error::Damaged)?;
    Ok(Sample {
        id: row.id,
        provider: Provider::parse(&row.provider).ok_or(Error::Damaged)?,
        account: row.account,
        key: row.sample_key,
        observed_at: row.observed_at,
        received_at: row.received_at,
        position: serde_json::from_slice(&plain).map_err(|_| Error::Damaged)?,
    })
}

async fn between(
    db: &PgPool,
    seal: &Seal,
    span: (DateTime<Utc>, DateTime<Utc>),
    limit: i64,
) -> Result<Vec<Sample>, Error> {
    let rows = store::samples_between(db, span, limit).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let key = seal.key()?;
    rows.into_iter().map(|row| opened(&key, row)).collect()
}

/// A sample with no time is in no span.
///
/// # Errors
/// `Invalid` when `to` is not after `from`, the span is longer than 31 days or `first` is out
/// of range; `Key`, `Damaged`, `Database`.
pub async fn samples(
    db: &PgPool,
    seal: &Seal,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    first: i64,
) -> Result<Vec<Sample>, Error> {
    let mut bad = Vec::new();
    if to <= from {
        bad.push(Bad::new("to", "Give a time after `from`."));
    } else if to - from > MAX_SPAN {
        bad.push(Bad::new("to", "Give a span of at most 31 days."));
    }
    if !(1..=MAX_READ).contains(&first) {
        bad.push(Bad::new("first", "Give a number from 1 to 1000."));
    }
    if !bad.is_empty() {
        return Err(Error::Invalid(bad));
    }
    between(db, seal, (from, to), first).await
}

/// The samples that the hub got after `after` and not after `until`, in the order of their
/// arrival, at most `limit` (K-E6 Section 4).
///
/// # Errors
/// `Key`, `Damaged`, `Database`.
pub async fn received(
    conn: &mut PgConnection,
    seal: &Seal,
    span: (DateTime<Utc>, DateTime<Utc>),
    limit: i64,
) -> Result<Vec<Sample>, Error> {
    let rows = store::samples_received(conn, span, limit).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let key = seal.key()?;
    rows.into_iter().map(|row| opened(&key, row)).collect()
}

fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(TimeDelta::hours(1)).unwrap_or(at)
}

/// One audit entry for each ingest token and each hour that ended before `now` (R-262). An hour
/// is written one time: its count leaves the table with the entry.
///
/// # Errors
/// `Database`, `Trail`.
pub async fn audit_closed_hours(conn: &mut PgConnection, now: DateTime<Utc>) -> Result<u64, Error> {
    let closed = store::take_closed_hours(conn, hour_of(now)).await?;
    let mut written = 0;
    for hour in closed {
        let entry = events::NewAudit {
            operator: events::Operator::hub(),
            action: events::AuditAction::Create,
            target: events::AuditTarget::IngestToken(hour.token_id),
            client: events::ClientAddr(hour.client),
            change: trail::hour_change(&hour.name, hour.hour, hour.fixes),
        };
        events::audit(conn, now, entry).await?;
        written += 1;
    }
    Ok(written)
}

/// # Errors
/// `Database`, `Trail`.
pub async fn audit_ended_hours(db: &PgPool, now: DateTime<Utc>) -> Result<u64, Error> {
    let mut tx = db.begin().await?;
    let written = audit_closed_hours(&mut tx, now).await?;
    tx.commit().await?;
    Ok(written)
}

pub(crate) async fn count_stored(
    conn: &mut PgConnection,
    token: crate::ids::IngestTokenId,
    stored: Stored,
    (now, client): (DateTime<Utc>, events::ClientAddr),
) -> Result<(), Error> {
    let fixes = i32::try_from(stored.added).unwrap_or(i32::MAX);
    if fixes > 0 {
        store::count_fixes(conn, (token, hour_of(now)), (fixes, client.0)).await?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Transfer {
    pub event: EventId,
    pub occurred_at: DateTime<Utc>,
    pub direction: Direction,
    pub device: (DeviceId, SiteId),
    pub actor: Actor,
}

/// The device side gets its site and the place of that site as they are now; the other side,
/// the computer of the client, stays unknown. The place of the hub is never written.
///
/// # Errors
/// `Places`, `Database`.
pub async fn record_transfer(conn: &mut PgConnection, transfer: Transfer) -> Result<(), Error> {
    let (device, site) = transfer.device;
    let known = End {
        device: Some(device),
        site: Some(site),
        place: places::place_of_site(conn, site).await?,
    };
    let (source, destination) = match transfer.direction {
        Direction::Download => (known, End::default()),
        Direction::Upload => (End::default(), known),
    };
    let stored = StoredContext {
        event: transfer.event,
        occurred_at: transfer.occurred_at,
        direction: transfer.direction,
        source,
        destination,
        actor: transfer.actor,
    };
    Ok(store::insert_context(conn, &stored).await?)
}

/// The evidence for the position of the operator is computed now from the samples near the
/// time of the event (R-145).
///
/// # Errors
/// `Key`, `Damaged`, `Places`, `Database`.
pub async fn transfer_context(
    db: &PgPool,
    seal: &Seal,
    event: EventId,
) -> Result<Option<(StoredContext, OperatorEvidence)>, Error> {
    let Some(stored) = store::context(db, event).await? else {
        return Ok(None);
    };
    let reach = TimeDelta::from_std(RULE.nearest).unwrap_or(TimeDelta::zero());
    let span = (stored.occurred_at - reach, stored.occurred_at + reach);
    let near = between(db, seal, span, NEAR_LIMIT).await?;
    let known = if near.is_empty() {
        Vec::new()
    } else {
        places::list(db).await?
    };
    let evidence = operator_evidence(stored.occurred_at, &near, &known, &RULE);
    Ok(Some((stored, evidence)))
}
