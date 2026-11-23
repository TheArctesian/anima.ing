//! When a failed notification is tried again (D-16). Pure.
//!
//! A claim counts the try before the send, so a try whose outcome was never written (the hub
//! stopped, or the write failed) still counts.

use chrono::{DateTime, TimeDelta, Utc};

use super::failure::Failure;

pub(super) const WAITS: [TimeDelta; 4] = [
    TimeDelta::minutes(1),
    TimeDelta::minutes(5),
    TimeDelta::minutes(15),
    TimeDelta::minutes(60),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Next {
    RetryAt(DateTime<Utc>),
    GiveUp,
}

const TRIES: usize = WAITS.len() + 1;

/// `tried` includes this try.
pub(super) fn after_failure(failure: Failure, tried: u32, now: DateTime<Utc>) -> Next {
    if failure.is_permanent() {
        return Next::GiveUp;
    }
    tried
        .checked_sub(1)
        .and_then(|failed_before| usize::try_from(failed_before).ok())
        .and_then(|failed_before| WAITS.get(failed_before))
        .map_or(Next::GiveUp, |wait| Next::RetryAt(now + *wait))
}

/// A send whose outcome cannot be written is not repeated without end (D-16).
pub(super) fn out_of_tries(tried: u32) -> bool {
    usize::try_from(tried).map_or(true, |tried| tried > TRIES)
}
