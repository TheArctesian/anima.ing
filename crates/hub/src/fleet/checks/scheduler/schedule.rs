//! The scheduler's decisions (CHK-001, CHK-002, CHK-010, DB-005, R-102); time and randomness are parameters.

use std::collections::HashSet;

use chrono::{DateTime, TimeDelta, Utc};
use rand::{Rng, RngExt as _};

use crate::{
    fleet::{
        checks::{
            config::CheckConfig,
            model::Due,
            probe::{Failure, ProbeResult},
        },
        events::Severity,
    },
    ids::CheckId,
    status::Status,
};

fn interval(seconds: u32) -> TimeDelta {
    TimeDelta::seconds(i64::from(seconds.max(1)))
}

/// Ticks are a second apart, and the wait starts when a slow tick ends.
const AWAKE_WAIT: TimeDelta = TimeDelta::seconds(30);

/// A hub that was suspended goes on with its old state, so only the clock shows it.
pub(super) fn slept(idle_since: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    idle_since.is_some_and(|since| now - since > AWAKE_WAIT)
}

/// A domain expiry check asks its RDAP server at most once an interval (R-102, CHK-009).
pub(super) fn floor(due: &Due, queried_at: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    let domain = matches!(due.config, Some(CheckConfig::DomainExpiry { .. }));
    queried_at
        .filter(|_| domain)
        .map(|at| at + interval(due.settings.interval_seconds))
}

pub(super) fn not_before_floor(
    due: &Due,
    queried_at: Option<DateTime<Utc>>,
    at: DateTime<Utc>,
) -> DateTime<Utc> {
    floor(due, queried_at).map_or(at, |floor| at.max(floor))
}

/// A check due before the scheduler started is moved, so a restart does not probe everything at once (R-102).
pub(super) fn needs_new_due(due: &Due, started_at: DateTime<Utc>) -> bool {
    let floor = floor(due, due.last_beat_at);
    due.state
        .next_due_at
        .is_none_or(|at| at < started_at || floor.is_some_and(|floor| at < floor))
}

/// Spread over one interval (CHK-001); a PUSH check waits all of it, as its device has had no chance to call.
pub(super) fn new_due(due: &Due, now: DateTime<Utc>, rng: &mut impl Rng) -> DateTime<Utc> {
    let interval = interval(due.settings.interval_seconds);
    if matches!(due.config, Some(CheckConfig::Push)) {
        return now + interval;
    }
    let millis = interval.num_milliseconds();
    let spread = now + TimeDelta::milliseconds(rng.random_range(0..millis));
    not_before_floor(due, due.last_beat_at, spread)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Timing {
    pub(super) next_due_at: Option<DateTime<Utc>>,
    pub(super) interval_seconds: u32,
}

impl From<&Due> for Timing {
    fn from(due: &Due) -> Self {
        Self {
            next_due_at: due.state.next_due_at,
            interval_seconds: due.settings.interval_seconds,
        }
    }
}

/// Lateness counts from `started_at` at the earliest, so the hub's own downtime is not reported (CHK-010).
pub(super) fn is_late(timing: Timing, started_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    timing
        .next_due_at
        .is_some_and(|at| at.max(started_at) < now - interval(timing.interval_seconds) * 2)
}

/// Each lateness episode gives one event (CHK-010).
#[derive(Debug, Default)]
pub(super) struct Watchdog {
    reported: HashSet<CheckId>,
}

impl Watchdog {
    pub(super) fn should_report(&mut self, check: CheckId, late: bool) -> bool {
        late && self.reported.insert(check)
    }

    pub(super) fn clear(&mut self, check: CheckId) {
        self.reported.remove(&check);
    }

    pub(super) fn retain(&mut self, kept: &HashSet<CheckId>) {
        self.reported.retain(|check| kept.contains(check));
    }
}

/// A missed push is a failed attempt, so retries apply (CHK-002).
pub(super) fn missed_push() -> ProbeResult {
    ProbeResult::failed(&Failure::NoPush)
}

pub(super) const fn severity_of(status: Status) -> Severity {
    match status {
        Status::Down => Severity::Error,
        Status::Degraded => Severity::Warning,
        Status::Up | Status::Maintenance | Status::Unknown => Severity::Info,
    }
}

const fn word(status: Status) -> &'static str {
    match status {
        Status::Up => "Up",
        Status::Down => "Down",
        Status::Degraded => "Degraded",
        Status::Maintenance => "in maintenance",
        Status::Unknown => "Unknown",
    }
}

/// The device name and the status only (LOG-007).
pub(super) fn status_message(device: &str, status: Status) -> String {
    format!("{device} is {}", word(status))
}

/// The check and device names only, never a host or a secret (SEC-010, LOG-007).
pub(super) fn moved_secrets_message(check: &str, device: &str) -> String {
    format!(
        "Check {check} on {device} stopped: the device host changed after its SNMP secrets \
         were entered, so the hub removed them and sends nothing. Enter the community or the \
         v3 passwords again"
    )
}

/// The check and device names only (LOG-007).
pub(super) fn late_message(check: &str, device: &str) -> String {
    format!("Check {check} on {device} is late by more than 2 intervals")
}
