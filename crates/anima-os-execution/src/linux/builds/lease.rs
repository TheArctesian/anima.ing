//! A hub lease is bounded again by the local boot clock before execution.
mod watch;
pub(in crate::linux) use watch::watch;

use super::{
    super::{Error, Result},
    store,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use uuid::Uuid;

pub use daemon_core::source_jobs::Grant;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub grant: Grant,
    boot: Uuid,
    deadline_ms: u64,
}

struct Clock {
    boot: Uuid,
    uptime_ms: u64,
    utc: DateTime<Utc>,
}

impl Clock {
    fn now() -> Result<Self> {
        let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
        let uptime = fs::read_to_string("/proc/uptime")?;
        let seconds = uptime
            .split_whitespace()
            .next()
            .and_then(|v| v.split_once('.'));
        let uptime_ms = seconds
            .and_then(|(whole, fraction)| {
                if fraction.len() != 2 || !fraction.bytes().all(|v| v.is_ascii_digit()) {
                    return None;
                }
                whole
                    .parse::<u64>()
                    .ok()?
                    .checked_mul(1000)?
                    .checked_add(fraction.parse::<u64>().ok()?.checked_mul(10)?)
            })
            .ok_or(Error::Refused("The build lease clock is unavailable."))?;
        Ok(Self {
            boot: Uuid::parse_str(boot.trim())
                .map_err(|_| Error::Refused("Invalid boot identity."))?,
            uptime_ms,
            utc: Utc::now(),
        })
    }
}

impl Lease {
    pub fn new(grant: Grant) -> Result<Self> {
        Self::at(grant, &Clock::now()?)
    }

    fn at(grant: Grant, clock: &Clock) -> Result<Self> {
        let remaining = grant
            .until
            .signed_duration_since(clock.utc)
            .num_milliseconds();
        if grant.job.is_nil() || grant.attempt == 0 || !(1..=90_000).contains(&remaining) {
            return Err(Error::Refused(
                "The build lease identity or lifetime is invalid.",
            ));
        }
        let remaining = u64::try_from(remaining)
            .map_err(|_| Error::Refused("Invalid build lease lifetime."))?;
        Ok(Self {
            grant,
            boot: clock.boot,
            deadline_ms: clock
                .uptime_ms
                .checked_add(remaining)
                .ok_or(Error::Refused("Build lease clock overflowed."))?,
        })
    }

    pub fn expired(&self) -> bool {
        Clock::now().map_or(true, |clock| self.expired_at(&clock))
    }

    fn expired_at(&self, clock: &Clock) -> bool {
        self.boot != clock.boot
            || clock.uptime_ms >= self.deadline_ms
            || clock.utc >= self.grant.until
    }

    fn renewed(&self, grant: Grant, clock: &Clock) -> Result<Self> {
        if self.expired_at(clock) || !self.matches(grant) || grant.until < self.grant.until {
            return Err(Error::Refused(
                "The build lease expired or changed ownership.",
            ));
        }
        if grant.until == self.grant.until {
            return Ok(Self {
                grant,
                boot: self.boot,
                deadline_ms: self.deadline_ms,
            });
        }
        Self::at(grant, clock)
    }

    pub fn extend(&self, grant: Grant) -> Result<Self> {
        self.renewed(grant, &Clock::now()?)
    }

    pub fn matches(&self, grant: Grant) -> bool {
        self.grant.job == grant.job && self.grant.attempt == grant.attempt
    }
}

pub fn expired_attempt(attempt: &store::Attempt) -> bool {
    matches!(
        attempt.phase.as_str(),
        "preparing" | "launching" | "running" | "unknown"
    ) && attempt.lease.as_ref().is_some_and(Lease::expired)
}

pub fn renew(id: Uuid, grant: Grant) -> Result<Value> {
    let _lock = store::lock(id)?;
    let mut attempt = store::required(id)?;
    renew_attempt(&mut attempt, grant)?;
    Ok(serde_json::to_value(attempt)?)
}

pub(in crate::linux) fn renew_supervised(id: Uuid, execution: Uuid, grant: Grant) -> Result<()> {
    let base = store::base(id);
    // Before staging, launch reads the renewal directly from the source journal.
    if !base.try_exists()? {
        return Ok(());
    }
    let _lock = store::lock(id)?;
    if !base.try_exists()? {
        return Ok(());
    }
    let mut attempt = store::required(id)?;
    if attempt.supervisor != Some(execution) {
        return Err(Error::Refused("The source job no longer owns this build."));
    }
    // The supervisor can renew its own lease before it records the verified result.
    if attempt.phase == "verified"
        && attempt
            .lease
            .as_ref()
            .is_some_and(|lease| lease.matches(grant))
    {
        return Ok(());
    }
    renew_attempt(&mut attempt, grant)
}

fn renew_attempt(attempt: &mut store::Attempt, grant: Grant) -> Result<()> {
    if !matches!(
        attempt.phase.as_str(),
        "preparing" | "launching" | "running" | "unknown"
    ) {
        return Err(Error::Refused(
            "This build no longer accepts lease renewal.",
        ));
    }
    let lease = attempt
        .lease
        .as_ref()
        .ok_or(Error::Refused("This build has no hub lease."))?;
    attempt.lease = Some(lease.extend(grant)?);
    store::save(attempt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_persisted_lease_cannot_be_extended_by_retry_clock_rollback_or_a_new_boot() {
        let now = DateTime::from_timestamp(1_790_000_000, 0).unwrap();
        let mut clock = Clock {
            boot: Uuid::now_v7(),
            uptime_ms: 1000,
            utc: now,
        };
        let grant = Grant {
            job: Uuid::now_v7(),
            attempt: 1,
            until: now + chrono::TimeDelta::seconds(60),
        };
        let lease = Lease::at(grant, &clock).unwrap();
        let bytes = serde_json::to_vec(&lease).unwrap();
        let restored: Lease = serde_json::from_slice(&bytes).unwrap();
        clock.uptime_ms += 30_000;
        clock.utc -= chrono::TimeDelta::seconds(10);
        let retry = restored.renewed(grant, &clock).unwrap();
        assert!(!retry.expired_at(&clock));
        assert!(
            retry
                .renewed(
                    Grant {
                        attempt: 2,
                        ..grant
                    },
                    &clock
                )
                .is_err()
        );
        let longer = Grant {
            until: now + chrono::TimeDelta::seconds(70),
            ..grant
        };
        let renewed = retry.renewed(longer, &clock).unwrap();
        clock.uptime_ms = 61_000;
        assert!(retry.expired_at(&clock));
        assert!(retry.renewed(longer, &clock).is_err());
        assert!(!renewed.expired_at(&clock));
        clock.boot = Uuid::now_v7();
        assert!(renewed.expired_at(&clock));
        clock.uptime_ms = 0;
        assert!(renewed.renewed(longer, &clock).is_err());
        assert!(
            Lease::at(
                Grant {
                    until: clock.utc + chrono::TimeDelta::seconds(91),
                    ..grant
                },
                &clock
            )
            .is_err()
        );
    }
}
