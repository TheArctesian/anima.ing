//! The drain of a node that a policy stops (CAP-013); the caller gives the time.

use chrono::{DateTime, TimeDelta, Utc};

pub const DRAIN_LIMIT: TimeDelta = TimeDelta::seconds(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drain {
    Serving,
    Draining { until: DateTime<Utc> },
    Stopped,
}

impl Drain {
    #[must_use]
    pub fn next(self, permitted: bool, now: DateTime<Utc>) -> Self {
        match (self, permitted) {
            (_, true) => Self::Serving,
            (Self::Serving, false) => Self::Draining {
                until: now + DRAIN_LIMIT,
            },
            (Self::Draining { until }, false) if now < until => self,
            (Self::Draining { .. } | Self::Stopped, false) => Self::Stopped,
        }
    }

    #[must_use]
    pub const fn takes_new(self) -> bool {
        matches!(self, Self::Serving)
    }

    #[must_use]
    pub fn runs_current(self, now: DateTime<Utc>) -> bool {
        match self {
            Self::Serving => true,
            Self::Draining { until } => now < until,
            Self::Stopped => false,
        }
    }
}
