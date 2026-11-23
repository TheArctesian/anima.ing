//! What each account may still spend of its rate limit on live reads (K-E1 Section 13.1;
//! decisions 5 and 7 of the slice).
//!
//! The reserve is a count of requests left, so it holds for any hourly total. The last answer
//! always wins, as a remaining count may rise from one answer to the next.

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};

use crate::{
    engine::sources::acquisition::verdict::{Limits, Verdict, judge},
    ids::SourceAccountId,
};

/// Live reads stop when this many requests or fewer are left: they belong to the sync.
pub(super) const RESERVE: u64 = 500;
const NO_RESET_WAIT: TimeDelta = TimeDelta::minutes(1);

/// Decision 7: a 429 always limits; a 403 only when no request is left or it names a
/// `retry-after`. Any other 403 is a refusal for the provider to read.
pub(super) fn limited_until(
    status: u16,
    limits: Limits,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    match judge(status, limits, now) {
        Verdict::RateLimited(until) => Some(until),
        Verdict::Usable | Verdict::Refused if status == 429 => Some(
            limits
                .reset
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
                .filter(|reset| *reset > now)
                .unwrap_or(now + NO_RESET_WAIT),
        ),
        Verdict::Usable | Verdict::Refused => None,
    }
}

#[derive(Clone, Copy, Debug)]
struct Seen {
    remaining: u64,
    reset_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub(super) struct Budget {
    seen: HashMap<SourceAccountId, Seen>,
}

impl Budget {
    pub(super) fn note(&mut self, account: SourceAccountId, limits: Limits, now: DateTime<Utc>) {
        let Some(remaining) = limits.remaining else {
            return;
        };
        let reset_at = limits
            .reset
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
            .unwrap_or(now + NO_RESET_WAIT);
        self.seen.insert(
            account,
            Seen {
                remaining,
                reset_at,
            },
        );
    }

    pub(super) fn limited(&mut self, account: SourceAccountId, until: DateTime<Utc>) {
        self.seen.insert(
            account,
            Seen {
                remaining: 0,
                reset_at: until,
            },
        );
    }

    /// The changes of the operator do not ask: they may spend the reserve.
    pub(super) fn read(
        &self,
        account: SourceAccountId,
        now: DateTime<Utc>,
    ) -> Result<(), DateTime<Utc>> {
        match self.seen.get(&account) {
            Some(seen) if seen.remaining <= RESERVE && now < seen.reset_at => Err(seen.reset_at),
            _ => Ok(()),
        }
    }
}
