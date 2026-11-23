//! Tries of a Keychain call that waited (R-322): quick at first, so a person who allows the
//! daemon sees it work soon, then slow, so a computer with nobody at it is not busy.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

const QUICK_TRIES: u32 = 20;
const QUICK: Duration = Duration::from_secs(30);
const SLOW: Duration = Duration::from_mins(5);

#[must_use]
pub const fn next_try(waited: u32) -> Duration {
    if waited < QUICK_TRIES { QUICK } else { SLOW }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tries {
    waited: u32,
    next_at: DateTime<Utc>,
}

impl Tries {
    #[must_use]
    pub fn first(now: DateTime<Utc>) -> Self {
        Self {
            waited: 1,
            next_at: now + pause(0),
        }
    }

    #[must_use]
    pub fn again(self, now: DateTime<Utc>) -> Self {
        Self {
            waited: self.waited.saturating_add(1),
            next_at: now + pause(self.waited),
        }
    }

    #[must_use]
    pub fn due(&self, now: DateTime<Utc>) -> bool {
        now >= self.next_at
    }
}

fn pause(waited: u32) -> TimeDelta {
    TimeDelta::from_std(next_try(waited)).unwrap_or(TimeDelta::minutes(5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn met_013_tries_come_every_30_s_for_10_minutes_then_every_5_minutes() {
        assert_eq!(next_try(0), Duration::from_secs(30));
        assert_eq!(next_try(19), Duration::from_secs(30));
        assert_eq!(next_try(20), Duration::from_mins(5));
        assert_eq!(next_try(u32::MAX), Duration::from_mins(5));

        let start = DateTime::UNIX_EPOCH;
        let mut tries = Tries::first(start);
        let mut now = start;
        for _ in 0..19 {
            assert!(!tries.due(now + TimeDelta::seconds(29)));
            now += TimeDelta::seconds(30);
            assert!(tries.due(now));
            tries = tries.again(now);
        }
        assert!(
            tries.due(now + TimeDelta::seconds(30)),
            "the 20th pause is 30 s"
        );
        now += TimeDelta::seconds(30);
        tries = tries.again(now);
        assert!(!tries.due(now + TimeDelta::seconds(299)));
        assert!(
            tries.due(now + TimeDelta::minutes(5)),
            "then every 5 minutes"
        );
    }
}
