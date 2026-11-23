//! What one probe result means for a check: retries, invert and the Degraded limit (CHK-002, CHK-008).

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

use super::probe::{Judgement, ProbeResult};
use crate::status::Status;

/// A check's common settings (spec 6.4); `Default` gives the contract defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckSettings {
    pub interval_seconds: u32,
    /// Failed retries after the first failed attempt before the check is Down (CHK-002).
    pub retries: u8,
    pub retry_interval_seconds: u32,
    pub degraded_threshold_ms: u32,
    /// A failed probe counts as a success and a success as a failure (CHK-008).
    pub invert: bool,
}

impl Default for CheckSettings {
    fn default() -> Self {
        Self {
            interval_seconds: 60,
            retries: 3,
            retry_interval_seconds: 20,
            degraded_threshold_ms: 1000,
            invert: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckState {
    pub status: Status,
    pub failures: u32,
    /// When the next probe is due; `None` means as soon as possible.
    pub next_due_at: Option<DateTime<Utc>>,
}

impl Default for CheckState {
    fn default() -> Self {
        Self {
            status: Status::Unknown,
            failures: 0,
            next_due_at: None,
        }
    }
}

fn after(now: DateTime<Utc>, seconds: u32) -> DateTime<Utc> {
    now + TimeDelta::seconds(i64::from(seconds))
}

/// The check's new state and the heartbeat status (CHK-002, CHK-008, D-2, D-29).
///
/// An inverted success has no response time, so it is never Degraded; an unjudged result is
/// Unknown and skips invert. Before Down, a failure keeps the status and records Unknown (pending).
#[must_use]
pub fn evaluate(
    prev: &CheckState,
    probe: &ProbeResult,
    cfg: &CheckSettings,
    now: DateTime<Utc>,
) -> (CheckState, Status) {
    if probe.judgement == Judgement::Unjudged {
        let state = CheckState {
            status: Status::Unknown,
            failures: 0,
            next_due_at: Some(after(now, cfg.interval_seconds)),
        };
        return (state, Status::Unknown);
    }
    let succeeded = probe.ok != cfg.invert;
    if succeeded {
        let response_time = if cfg.invert {
            None
        } else {
            probe.response_time
        };
        let limit = Duration::from_millis(u64::from(cfg.degraded_threshold_ms));
        let warned = matches!(probe.judgement, Judgement::Starting | Judgement::Expiring);
        let status = if warned || response_time.is_some_and(|time| time > limit) {
            Status::Degraded
        } else {
            Status::Up
        };
        let state = CheckState {
            status,
            failures: 0,
            next_due_at: Some(after(now, cfg.interval_seconds)),
        };
        return (state, status);
    }
    let failures = prev.failures.saturating_add(1);
    if failures > u32::from(cfg.retries) {
        let state = CheckState {
            status: Status::Down,
            failures,
            next_due_at: Some(after(now, cfg.interval_seconds)),
        };
        return (state, Status::Down);
    }
    let state = CheckState {
        status: prev.status,
        failures,
        next_due_at: Some(after(now, cfg.retry_interval_seconds)),
    };
    (state, Status::Unknown)
}
