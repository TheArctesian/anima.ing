//! The return of a displaced pin (K11 Section 11.1, FIT-023): when a pinned copy that was unloaded to make
//! room loads again by itself, with no loop of loads and no wait for a request.

use std::collections::BTreeMap;

use uuid::Uuid;

use super::room::{Load, Room, Usable, room};
use super::{Held, Memory, OnServer, part_on};

/// The first interval between two returns of a copy, and the time a return must stay loaded to reset it.
pub const FIRST_INTERVAL: u64 = 600;
/// The longest interval.
pub const LONGEST_INTERVAL: u64 = 4 * 3600;
/// No return on a memory for this long after a `memory_busy` on it.
pub const QUIET_AFTER_BUSY: u64 = 60;

/// A pinned copy that waits for its return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Displaced {
    pub copy: Uuid,
    pub load: Load,
    /// The second of its last return.
    pub returned_at: Option<u64>,
    pub interval: u64,
}

impl Displaced {
    /// A copy displaced at `now`; a copy that was displaced less than [`FIRST_INTERVAL`] after its return
    /// doubles its interval, up to [`LONGEST_INTERVAL`].
    #[must_use]
    pub fn new(copy: Uuid, load: Load, last: Option<(u64, u64)>, now: u64) -> Self {
        let (returned_at, interval) = match last {
            Some((at, interval)) if now.saturating_sub(at) < FIRST_INTERVAL => (
                Some(at),
                interval
                    .saturating_mul(2)
                    .clamp(FIRST_INTERVAL, LONGEST_INTERVAL),
            ),
            Some((at, _)) => (Some(at), FIRST_INTERVAL),
            None => (None, FIRST_INTERVAL),
        };
        Self {
            copy,
            load,
            returned_at,
            interval,
        }
    }

    fn interval_passed(&self, now: u64) -> bool {
        self.returned_at
            .is_none_or(|at| now.saturating_sub(at) >= self.interval)
    }
}

/// What the node is doing now, for the choice of a return.
pub struct Now<'a> {
    pub usable: &'a Usable,
    pub held: &'a [Held],
    /// The last `memory_busy` of each memory.
    pub busy_at: &'a BTreeMap<Memory, u64>,
    /// The switch is on and the device policy permits model work now.
    pub permitted: bool,
}

impl Now<'_> {
    fn quiet(&self, memory: Memory, now: u64) -> bool {
        let loading = self
            .held
            .iter()
            .any(|copy| copy.state == OnServer::Loading && part_on(&copy.parts, memory) > 0);
        let busy = self
            .busy_at
            .get(&memory)
            .is_some_and(|&at| now.saturating_sub(at) < QUIET_AFTER_BUSY);
        !loading && !busy
    }
}

/// The first displaced copy that may return now. The caller passes only copies whose server is `ready` (for a
/// runtime that cannot unload, with no copy) and whose checks of a load pass; a return never unloads.
#[must_use]
pub fn next(node: &Now<'_>, displaced: &[Displaced], now: u64) -> Option<Uuid> {
    if !node.permitted {
        return None;
    }
    displaced
        .iter()
        .find(|copy| {
            copy.interval_passed(now)
                && copy
                    .load
                    .parts
                    .iter()
                    .all(|&(memory, _)| node.quiet(memory, now))
                && room(node.usable, node.held, &copy.load, now) == Room::Load
        })
        .map(|copy| copy.copy)
}
