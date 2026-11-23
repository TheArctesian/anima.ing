//! Which aggregate table and which buckets a chart range reads (MET-003).

use chrono::{DateTime, DurationRound, TimeDelta, Utc};

use super::MetricRange;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resolution {
    Minute,
    Hour,
}

/// `bin` is a whole number of seconds that divides an hour or is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Window {
    pub(super) resolution: Resolution,
    pub(super) start: DateTime<Utc>,
    pub(super) end: DateTime<Utc>,
    pub(super) bin: TimeDelta,
}

/// The oldest bin starts on a bin boundary, so every bin holds all of its minutes.
pub(super) fn window(range: MetricRange, now: DateTime<Utc>) -> Window {
    let (resolution, bin, points) = match range {
        MetricRange::Hour => (Resolution::Minute, TimeDelta::minutes(1), 60),
        MetricRange::Day => (Resolution::Minute, TimeDelta::minutes(10), 144),
        MetricRange::Week => (Resolution::Hour, TimeDelta::hours(1), 168),
    };
    let newest = now.duration_trunc(bin).unwrap_or(now);
    Window {
        resolution,
        start: newest - bin * (points - 1),
        end: now,
        bin,
    }
}
