//! When an account syncs next, and which due accounts the scheduler runs (SRC-021, K-E1
//! Section 10).

use chrono::{DateTime, TimeDelta, Utc};
use source_exchange::MIN_POLL_INTERVAL_SECONDS;

use crate::ids::SourceAccountId;

pub const MAX_BACKOFF: TimeDelta = TimeDelta::hours(6);

/// Never under the 15-minute floor (SRC-021).
#[must_use]
pub fn interval(declared_seconds: u32) -> TimeDelta {
    TimeDelta::seconds(i64::from(declared_seconds.max(MIN_POLL_INTERVAL_SECONDS)))
}

/// `failure_count` is the count after the run: 0 after a success.
#[must_use]
pub fn next(
    now: DateTime<Utc>,
    interval: TimeDelta,
    failure_count: u32,
    rate_limited_until: Option<DateTime<Utc>>,
) -> DateTime<Utc> {
    let wait = if failure_count == 0 {
        interval
    } else {
        // 2^16 intervals is far past the cap, so a longer run of failures cannot overflow.
        let factor = 1_i32 << failure_count.min(16);
        (interval * factor).min(MAX_BACKOFF).max(interval)
    };
    let next = now + wait;
    rate_limited_until.map_or(next, |until| next.max(until))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Due {
    pub id: SourceAccountId,
    pub source: String,
}

/// An account with no consumer on is left as it is, so its `next_sync_at` is not touched
/// (EXT-002).
#[must_use]
pub fn runnable(
    due: &[Due],
    consumer_on: impl Fn(&str) -> bool,
    room: usize,
) -> Vec<SourceAccountId> {
    due.iter()
        .filter(|account| consumer_on(&account.source))
        .take(room)
        .map(|account| account.id)
        .collect()
}
