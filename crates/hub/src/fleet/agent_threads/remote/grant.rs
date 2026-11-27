use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use super::{Grant, Launch};

const LIFETIME: TimeDelta = TimeDelta::seconds(10);

#[derive(Clone, Copy)]
pub(super) struct Tick {
    pub generation: Uuid,
    pub nanoseconds: i64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn tick() -> Option<Tick> {
    use std::sync::OnceLock;

    static GENERATION: OnceLock<Uuid> = OnceLock::new();
    use nix::time::{ClockId, clock_gettime};

    #[cfg(target_os = "linux")]
    let clock = ClockId::CLOCK_BOOTTIME;
    // Apple's CLOCK_MONOTONIC_RAW calls mach_continuous_time, which includes sleep.
    #[cfg(target_os = "macos")]
    let clock = ClockId::from_raw(nix::libc::CLOCK_MONOTONIC_RAW);
    let time = clock_gettime(clock).ok()?;
    Some(Tick {
        generation: *GENERATION.get_or_init(Uuid::now_v7),
        nanoseconds: time
            .tv_sec()
            .checked_mul(1_000_000_000)?
            .checked_add(time.tv_nsec())?,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) const fn tick() -> Option<Tick> {
    None
}

pub(super) fn expiry(
    at: DateTime<Utc>,
    lease: DateTime<Utc>,
    deadline: DateTime<Utc>,
) -> DateTime<Utc> {
    (at + LIFETIME).min(lease).min(deadline)
}

pub(super) fn remaining(launch: &Launch, at: DateTime<Utc>, sample: Tick) -> Option<Grant> {
    let valid_until = launch.valid_until?;
    let original = valid_until.signed_duration_since(launch.issued_at?);
    let stamped = Tick {
        generation: launch.clock_generation?,
        nanoseconds: launch.issued_tick?,
    };
    let wall_end = valid_until.min(launch.lease_end?).min(launch.deadline);
    let remaining_ms = budget(
        original,
        wall_end.signed_duration_since(at),
        stamped,
        sample,
    )?;
    Some(Grant {
        authorization: launch.authorization?,
        valid_until,
        remaining_ms,
    })
}

fn budget(original: TimeDelta, wall: TimeDelta, stamped: Tick, sample: Tick) -> Option<i64> {
    let tick = sample.nanoseconds.checked_sub(stamped.nanoseconds)?;
    if sample.generation != stamped.generation || tick < 0 {
        return None;
    }
    let elapsed_left = original.checked_sub(&TimeDelta::nanoseconds(tick))?;
    let remaining = elapsed_left.min(wall).num_milliseconds();
    (remaining > 0).then_some(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_cannot_extend_the_duration_or_cross_a_clock_generation() {
        let original = TimeDelta::seconds(10);
        let renewed = TimeDelta::seconds(90);
        let start = Tick {
            generation: Uuid::from_u128(1),
            nanoseconds: 0,
        };
        let retry = Tick {
            nanoseconds: 9_998_100_000,
            ..start
        };
        let too_late = Tick {
            nanoseconds: 9_999_100_000,
            ..start
        };
        assert_eq!(budget(original, renewed, start, retry), Some(1));
        assert_eq!(budget(original, renewed, start, too_late), None);
        assert_eq!(budget(original, renewed, retry, start), None);
        let restarted = Tick {
            generation: Uuid::from_u128(2),
            ..retry
        };
        assert_eq!(budget(original, renewed, start, restarted), None);
    }
}
