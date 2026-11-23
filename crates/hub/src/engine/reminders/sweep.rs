//! The sweep of each minute (K-E6 Sections 3 and 4; REM-002, REM-006).

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};

use super::{
    Error,
    model::{Occurrence, PlaceChange, Trigger},
    occurrences::{self, Extensions},
    place_state::place_changes,
    sending::{WasKnown, fire, notice},
    store,
};
use crate::{
    engine::{locations, places, sources},
    fleet::{credentials::operational, events},
};

const SAMPLES: &str = "locations";
const EVENTS: &str = "events";
const BATCH: i64 = 5000;
/// Older samples fire no place rule (R-145).
const FIRST_REACH: TimeDelta = TimeDelta::minutes(10);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    /// Firings stored, pending or suppressed.
    pub firings: u64,
    pub place_changes: usize,
}

async fn fire_rules(
    conn: &mut PgConnection,
    wanted: impl Fn(&Trigger) -> bool,
    changes: &[PlaceChange],
    now: DateTime<Utc>,
) -> Result<u64, Error> {
    let known = WasKnown::read(conn, now).await?;
    let mut firings = 0;
    for stored in store::enabled_rules(conn).await? {
        if wanted(&stored.rule.trigger) {
            firings += fire(conn, (&stored.rule, &known), (&[], changes), now).await?;
        }
    }
    Ok(firings)
}

async fn times(db: &PgPool, now: DateTime<Utc>) -> Result<u64, Error> {
    let mut tx = db.begin().await?;
    store::lock_sweep(&mut tx).await?;
    let firings = fire_rules(
        &mut tx,
        |trigger| matches!(trigger, Trigger::Time(_)),
        &[],
        now,
    )
    .await?;
    tx.commit().await?;
    Ok(firings)
}

/// Drops the last group of a full batch, which it may have cut; the next sweep reads it whole.
fn whole_instants(mut samples: Vec<locations::Sample>) -> Vec<locations::Sample> {
    let full = i64::try_from(samples.len()).is_ok_and(|read| read == BATCH);
    let last = samples.last().map(|sample| sample.received_at);
    if full && last != samples.first().map(|sample| sample.received_at) {
        samples.retain(|sample| Some(sample.received_at) != last);
    }
    samples
}

async fn arrivals(db: &PgPool, seal: &locations::Seal, now: DateTime<Utc>) -> Result<Swept, Error> {
    let known_places = places::list(db).await?;
    let mut tx = db.begin().await?;
    store::lock_sweep(&mut tx).await?;
    let after = store::cursor(&mut tx, SAMPLES).await?;
    let after = after.unwrap_or(now - FIRST_REACH);
    let read = locations::received(&mut tx, seal, (after, now), BATCH).await?;
    let samples = whole_instants(read);
    let Some(newest) = samples.iter().map(|sample| sample.received_at).max() else {
        return Ok(Swept::default());
    };
    let states = store::place_states(&mut tx).await?;
    let changes = place_changes(&states, &samples, &known_places);
    for change in &changes {
        store::put_place_state(&mut tx, change).await?;
    }
    let is_place = |trigger: &Trigger| matches!(trigger, Trigger::Place { .. });
    let firings = fire_rules(&mut tx, is_place, &changes, now).await?;
    store::set_cursor(&mut tx, SAMPLES, newest).await?;
    tx.commit().await?;
    Ok(Swept {
        firings,
        place_changes: changes.len(),
    })
}

#[derive(Clone)]
pub struct Reading {
    pub seal: locations::Seal,
    pub extensions: Extensions,
    pub sources: sources::Registry,
}

async fn pulled(
    db: &PgPool,
    extensions: &Extensions,
    now: DateTime<Utc>,
) -> Result<Vec<Occurrence>, Error> {
    let keys = operational::keys(db, now).await?;
    let mut new = occurrences::of_keys(&keys);
    let on = crate::engine::extensions::on_keys(db).await?;
    let read = extensions
        .iter()
        .filter(|extension| on.iter().any(|key| key.as_str() == extension.name()));
    for extension in read {
        new.extend(extension.expiries(db, now).await?);
        new.extend(extension.measures(db, now).await?);
    }
    Ok(new)
}

async fn given(db: &PgPool, reading: &Reading, now: DateTime<Utc>) -> Result<(), Error> {
    let seen = occurrences::seen(sources::healths(db, &reading.sources, now).await?);
    let mut new = pulled(db, &reading.extensions, now).await?;
    let mut tx = db.begin().await?;
    store::lock_sweep(&mut tx).await?;
    // The first sweep starts at now: an old event fires nothing.
    let after = store::cursor(&mut tx, EVENTS).await?.unwrap_or(now);
    let events = events::after(&mut tx, (after, now), BATCH).await?;
    let newest = events.last().map_or(now, |event| event.at);
    new.extend(events.iter().filter_map(occurrences::of_event));
    let before = store::states(&mut tx).await?;
    new.extend(occurrences::of_health(&before, &seen, now));
    for account in &seen {
        store::remember_state(&mut tx, (&account.account, account.state), now).await?;
    }
    notice(&mut tx, &new, now).await?;
    store::set_cursor(&mut tx, EVENTS, newest.max(after)).await?;
    tx.commit().await?;
    Ok(())
}

/// The time rules are stored also when the samples cannot be read.
///
/// # Errors
/// `Database`, `Places`, `Locations`, `Keys`, `Extension`, `Switches`, `Events`.
pub async fn sweep(db: &PgPool, reading: &Reading, now: DateTime<Utc>) -> Result<Swept, Error> {
    let timed = times(db, now).await?;
    given(db, reading, now).await?;
    let placed = arrivals(db, &reading.seal, now).await?;
    Ok(Swept {
        firings: timed + placed.firings,
        place_changes: placed.place_changes,
    })
}
