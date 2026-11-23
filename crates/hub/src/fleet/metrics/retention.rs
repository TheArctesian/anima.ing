//! Metric retention (DB-005, DB-009, HUB-005).

use chrono::{DateTime, DurationRound, NaiveDate, TimeDelta, Utc};

/// Defaults are DB-009's. A `samples` period under 1 day fails the run: the database refuses
/// to drop a sample partition less than 1 day old.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    pub samples: TimeDelta,
    pub minute: TimeDelta,
    pub hour: TimeDelta,
    pub day: TimeDelta,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            samples: TimeDelta::days(30),
            minute: TimeDelta::days(7),
            hour: TimeDelta::days(90),
            day: TimeDelta::days(730),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cutoffs {
    pub(super) samples: NaiveDate,
    pub(super) minute: DateTime<Utc>,
    pub(super) hour: DateTime<Utc>,
    pub(super) day: DateTime<Utc>,
}

impl Retention {
    pub(super) fn cutoffs(self, now: DateTime<Utc>) -> Cutoffs {
        Cutoffs {
            samples: (now - self.samples).date_naive(),
            minute: now - self.minute,
            hour: now - self.hour,
            day: now - self.day,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trimmed {
    pub partitions: Vec<String>,
    pub minute: u64,
    pub hour: u64,
    pub day: u64,
    /// Samples whose series no longer exists (DEV-015).
    pub orphan_samples: u64,
    /// Series with no aggregate row and no recent sample (D-5).
    pub idle_series: u64,
}

/// Partitions start here so the database's window (7 days past its clock) holds when `now` is ahead.
pub(super) fn start_of_day(now: DateTime<Utc>) -> DateTime<Utc> {
    now.duration_trunc(TimeDelta::days(1)).unwrap_or(now)
}
