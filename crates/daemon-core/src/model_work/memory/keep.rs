//! The idle time and the pin of a copy, and its reservation (K11 Section 11.1, FIT-022 to FIT-024).

use std::ops::RangeInclusive;

/// The permitted idle times, in seconds.
pub const IDLE_SECONDS: RangeInclusive<u32> = 60..=86_400;
/// The idle time of a copy that the node holds no value for: 15 minutes.
pub const DEFAULT_IDLE_SECONDS: u32 = 900;
/// The largest `reserveBytes`: the largest integer that JSON keeps exact.
pub const MAX_RESERVE_BYTES: u64 = (1 << 53) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keep {
    pub idle_seconds: u32,
    pub pinned: bool,
}

impl Default for Keep {
    fn default() -> Self {
        Self {
            idle_seconds: DEFAULT_IDLE_SECONDS,
            pinned: false,
        }
    }
}

/// The reservation of a copy: the larger of the hub's `reserve_bytes` and the daemon's own lower estimate,
/// which is the files and a quarter more, and at least `share_floor` (the share of a GPU that vLLM takes).
/// A wrong hub can make it larger, never smaller.
#[must_use]
pub fn reservation(files_bytes: u64, reserve_bytes: Option<u64>, share_floor: u64) -> u64 {
    let own = files_bytes.saturating_add(files_bytes / 4).max(share_floor);
    reserve_bytes.map_or(own, |hub| hub.max(own))
}
