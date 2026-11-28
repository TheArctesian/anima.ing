//! Timing at the holder boundary, in one suspend-inclusive clock domain.

use nix::time::ClockId;
use std::time::Duration;

// Allow a combined 2,000 ppm clock rate difference over these short intervals, in
// addition to the reported sample resolution. A backwards sample is unusable.
const RATE_DIVISOR: u128 = 500;

#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub(super) ticks: Duration,
    pub(super) resolution: Duration,
}

#[cfg(target_os = "macos")]
const CLOCK: ClockId = ClockId::from_raw(nix::libc::CLOCK_MONOTONIC_RAW);
#[cfg(target_os = "linux")]
const CLOCK: ClockId = ClockId::CLOCK_BOOTTIME;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn sample() -> Option<Sample> {
    // Darwin MONOTONIC_RAW uses mach_continuous_time. Linux BOOTTIME includes suspend.
    let resolution = CLOCK.res().ok()?;
    let ticks = CLOCK.now().ok()?;
    let duration = |value: nix::sys::time::TimeSpec| {
        let seconds = u64::try_from(value.tv_sec()).ok()?;
        let nanos = u32::try_from(value.tv_nsec()).ok()?;
        (nanos < 1_000_000_000).then(|| Duration::new(seconds, nanos))
    };
    Some(Sample {
        ticks: duration(ticks)?,
        resolution: duration(resolution)?,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) const fn sample() -> Option<Sample> {
    None
}

impl Sample {
    pub(super) fn before(self, now: Self, remaining_ms: u64) -> bool {
        let Some(elapsed) = now.ticks.checked_sub(self.ticks) else {
            return false;
        };
        let elapsed = elapsed.as_nanos();
        let uncertainty = self.resolution.max(now.resolution).as_nanos();
        let upper = elapsed + uncertainty + (elapsed + uncertainty).div_ceil(RATE_DIVISOR);
        upper.div_ceil(1_000_000) < u128::from(remaining_ms)
    }
}
