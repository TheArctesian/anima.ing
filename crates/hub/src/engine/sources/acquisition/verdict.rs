//! What a source's status says about access, before any provider reads the body (K-E1 Section 7).

use chrono::{DateTime, TimeDelta, Utc};
use reqwest::header::HeaderMap;

const DEFAULT_WAIT: TimeDelta = TimeDelta::minutes(1);
const LONGEST_WAIT: TimeDelta = TimeDelta::hours(24);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    pub remaining: Option<u64>,
    /// `x-ratelimit-reset`, in seconds since the epoch.
    pub reset: Option<i64>,
    /// `retry-after`, in seconds.
    pub retry_after: Option<u64>,
}

fn number<T: std::str::FromStr>(headers: &HeaderMap, name: &str) -> Option<T> {
    headers.get(name)?.to_str().ok()?.trim().parse().ok()
}

impl Limits {
    /// A header that is not a number counts as absent.
    #[must_use]
    pub fn of(headers: &HeaderMap) -> Self {
        Self {
            remaining: number(headers, "x-ratelimit-remaining"),
            reset: number(headers, "x-ratelimit-reset"),
            retry_after: number(headers, "retry-after"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Hand it to the provider, whatever its status.
    Usable,
    Refused,
    RateLimited(DateTime<Utc>),
}

#[must_use]
pub fn judge(status: u16, limits: Limits, now: DateTime<Utc>) -> Verdict {
    match status {
        401 => Verdict::Refused,
        403 | 429 if limits.remaining == Some(0) || limits.retry_after.is_some() => {
            let after = limits
                .retry_after
                .and_then(|seconds| i64::try_from(seconds).ok())
                .map(|seconds| now + TimeDelta::seconds(seconds.min(LONGEST_WAIT.num_seconds())));
            let reset = limits
                .reset
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0));
            let until = after.or(reset).unwrap_or(now + DEFAULT_WAIT);
            Verdict::RateLimited(until.clamp(now, now + LONGEST_WAIT))
        }
        _ => Verdict::Usable,
    }
}
