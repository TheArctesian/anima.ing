//! Room for a load, and the unload of idle copies (K11 Section 11.1, FIT-020, FIT-021, FIT-022, FIT-024).

use std::collections::BTreeMap;

use uuid::Uuid;

use super::{Held, Memory, OnServer, Parts, part_on, reserved_on};

/// A `ready` copy younger than this is not taken to make room, unless a return loaded it and nothing used it.
pub const SETTLE_SECONDS: u64 = 30;

/// A load that wants room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Load {
    pub parts: Parts,
}

/// What the daemon does for a load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Room {
    /// It fits now.
    Load,
    /// Unload these copies, in this order, then load.
    Unload(Vec<Uuid>),
    /// The memory is in use now (`memory_busy`); unload nothing.
    Busy,
    /// No unload can make room (`memory_share`).
    TooLarge,
}

/// The usable memory of each memory of the node; a memory that is not listed has none.
pub type Usable = BTreeMap<Memory, u64>;

fn usable_of(usable: &Usable, memory: Memory) -> u64 {
    usable.get(&memory).copied().unwrap_or(0)
}

/// The memories of `load` whose reservations, less `freed`, leave no room for it.
fn too_full(usable: &Usable, held: &[Held], load: &Load, freed: &[&Held]) -> Vec<Memory> {
    load.parts
        .iter()
        .filter(|&&(memory, part)| {
            let freed_here = freed.iter().fold(0u64, |sum, copy| {
                sum.saturating_add(part_on(&copy.parts, memory))
            });
            let after = reserved_on(held, memory)
                .saturating_sub(freed_here)
                .saturating_add(part);
            after > usable_of(usable, memory)
        })
        .map(|&(memory, _)| memory)
        .collect()
}

/// A copy that step 3 may take: `ready`, settled or returned and unused.
fn takeable(copy: &Held, now: u64) -> bool {
    copy.state == OnServer::Ready
        && (copy.returned_unused || now.saturating_sub(copy.used_at) >= SETTLE_SECONDS)
}

/// The rule of Section 11.1, steps 1 to 4.
#[must_use]
pub fn room(usable: &Usable, held: &[Held], load: &Load, now: u64) -> Room {
    if load
        .parts
        .iter()
        .any(|&(memory, part)| part > usable_of(usable, memory))
    {
        return Room::TooLarge;
    }
    let full = too_full(usable, held, load, &[]);
    if full.is_empty() {
        return Room::Load;
    }
    let mut candidates: Vec<&Held> = held
        .iter()
        .filter(|copy| {
            takeable(copy, now) && full.iter().any(|&memory| part_on(&copy.parts, memory) > 0)
        })
        .collect();
    candidates.sort_by_key(|copy| (copy.keep.pinned, copy.used_at));
    let mut taken: Vec<&Held> = Vec::new();
    for copy in candidates {
        taken.push(copy);
        if too_full(usable, held, load, &taken).is_empty() {
            return Room::Unload(taken.iter().map(|copy| copy.copy).collect());
        }
    }
    Room::Busy
}

/// The copies that are `ready`, not pinned, and unused for their idle time (FIT-022).
#[must_use]
pub fn idle(held: &[Held], now: u64) -> Vec<Uuid> {
    held.iter()
        .filter(|copy| {
            copy.state == OnServer::Ready
                && !copy.keep.pinned
                && now.saturating_sub(copy.used_at) >= u64::from(copy.keep.idle_seconds)
        })
        .map(|copy| copy.copy)
        .collect()
}
