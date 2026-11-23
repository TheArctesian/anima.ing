//! When a daily upkeep job runs (D-4, DB-004, DB-005).

use std::{future::Future, sync::Arc, time::Duration};

use chrono::{DateTime, NaiveTime, TimeDelta, Utc};

use crate::clock::Clock;

/// Partitions exist days ahead, so the time is not tied to the day rollover.
const DAILY_AT: NaiveTime = NaiveTime::from_hms_opt(0, 10, 0).expect("a valid time of day");
const FIRST_RETRY: Duration = Duration::from_secs(30);
const LONGEST_RETRY: Duration = Duration::from_mins(30);
const RETRIES: u32 = 8;

pub(super) fn next_daily_run(now: DateTime<Utc>) -> DateTime<Utc> {
    let today = now.date_naive().and_time(DAILY_AT).and_utc();
    if today > now {
        today
    } else {
        today + TimeDelta::days(1)
    }
}

pub(super) fn retry_delay(failures: u32) -> Option<Duration> {
    if failures == 0 || failures > RETRIES {
        return None;
    }
    let doubled = FIRST_RETRY.saturating_mul(1 << (failures - 1));
    Some(doubled.min(LONGEST_RETRY))
}

pub(super) fn plan(now: DateTime<Utc>, failures: u32) -> (Duration, u32) {
    let until_daily = (next_daily_run(now) - now)
        .to_std()
        .unwrap_or(Duration::ZERO);
    match retry_delay(failures) {
        Some(retry) => (retry.min(until_daily), failures),
        None => (until_daily, 0),
    }
}

/// A failure is logged as `name` and the fault word only, never its error text (LOG-007, API-010).
/// Dropping the future is safe: an unfinished transaction rolls back.
pub async fn run_daily<J, F>(name: &'static str, clock: Arc<dyn Clock>, mut job: J)
where
    J: FnMut(DateTime<Utc>) -> F,
    F: Future<Output = Result<(), &'static str>>,
{
    let mut failures = 0;
    loop {
        failures = match job(clock.now()).await {
            Ok(()) => 0,
            Err(fault) => {
                let failures = failures + 1;
                tracing::warn!(job = name, fault, failures, "daily job failed");
                failures
            }
        };
        let (wait, carried) = plan(clock.now(), failures);
        failures = carried;
        tokio::time::sleep(wait).await;
    }
}

#[cfg(test)]
mod tests;
