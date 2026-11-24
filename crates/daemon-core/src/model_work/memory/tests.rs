//! The acceptance of C2 for the node (`docs/work-items/compute/C2.md`, "Foundation acceptance"), with a test clock.

use std::collections::BTreeMap;

use uuid::Uuid;

use super::keep::{Keep, reservation};
use super::returns::{Displaced, FIRST_INTERVAL, LONGEST_INTERVAL, Now, next};
use super::room::{Load, Room, Usable, idle, room};
use super::{GpuShare, Held, Memory, OnServer, llama_parts};

const GIB: u64 = 1 << 30;

fn copy(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn pinned(mut held: Held) -> Held {
    held.keep.pinned = true;
    held
}

fn on_main(n: u8, gib: u64, state: OnServer, used_at: u64) -> Held {
    Held {
        copy: copy(n),
        state,
        reserved: gib * GIB,
        parts: vec![(Memory::Main, gib * GIB)],
        keep: Keep {
            idle_seconds: 900,
            pinned: false,
        },
        used_at,
        returned_unused: false,
    }
}

fn main_load(gib: u64) -> Load {
    Load {
        parts: vec![(Memory::Main, gib * GIB)],
    }
}

fn usable(gib: u64) -> Usable {
    BTreeMap::from([(Memory::Main, gib * GIB)])
}

#[test]
fn a_load_that_fits_unloads_nothing() {
    let held = [on_main(1, 8, OnServer::Ready, 0)];
    assert_eq!(room(&usable(24), &held, &main_load(8), 100), Room::Load);
}

#[test]
fn a_larger_load_takes_idle_memory_and_never_active_work() {
    let held = [
        on_main(1, 8, OnServer::Active, 0),
        on_main(2, 8, OnServer::Ready, 10),
        on_main(3, 6, OnServer::Ready, 20),
    ];
    // 22 of 24 held; a load of 12 needs 10 freed: the active copy stays, the two idle copies go, oldest first.
    assert_eq!(
        room(&usable(24), &held, &main_load(12), 1000),
        Room::Unload(vec![copy(2), copy(3)])
    );
}

#[test]
fn an_idle_pinned_copy_gives_its_memory_after_the_copies_that_are_not_pinned() {
    let held = [
        pinned(on_main(1, 10, OnServer::Ready, 0)),
        on_main(2, 10, OnServer::Ready, 500),
    ];
    assert_eq!(
        room(&usable(24), &held, &main_load(10), 1000),
        Room::Unload(vec![copy(2)])
    );
    assert_eq!(
        room(&usable(24), &held, &main_load(20), 1000),
        Room::Unload(vec![copy(2), copy(1)])
    );
}

#[test]
fn busy_memory_is_not_the_same_answer_as_a_copy_that_is_too_large() {
    let held = [on_main(1, 20, OnServer::Active, 0)];
    assert_eq!(room(&usable(24), &held, &main_load(8), 1000), Room::Busy);
    assert_eq!(room(&usable(24), &[], &main_load(30), 1000), Room::TooLarge);
}

#[test]
fn a_copy_that_just_finished_a_request_is_not_taken_at_once() {
    let held = [on_main(1, 20, OnServer::Ready, 990)];
    assert_eq!(room(&usable(24), &held, &main_load(8), 1000), Room::Busy);
    assert_eq!(
        room(&usable(24), &held, &main_load(8), 1020),
        Room::Unload(vec![copy(1)])
    );
}

#[test]
fn a_copy_that_a_return_loaded_and_nothing_used_can_be_taken_at_once() {
    let mut returned = pinned(on_main(1, 20, OnServer::Ready, 995));
    returned.returned_unused = true;
    assert_eq!(
        room(&usable(24), &[returned], &main_load(8), 1000),
        Room::Unload(vec![copy(1)])
    );
}

#[test]
fn the_idle_time_unloads_only_ready_copies_that_are_not_pinned() {
    let mut short = on_main(1, 4, OnServer::Ready, 0);
    short.keep.idle_seconds = 60;
    let held = [
        short,
        on_main(2, 4, OnServer::Ready, 0),
        pinned(on_main(3, 4, OnServer::Ready, 0)),
        on_main(4, 4, OnServer::Active, 0),
    ];
    assert_eq!(idle(&held, 120), vec![copy(1)]);
    assert_eq!(idle(&held, 900), vec![copy(1), copy(2)]);
}

#[test]
fn a_wrong_hub_can_make_the_reservation_larger_never_smaller() {
    assert_eq!(reservation(8 * GIB, None, 0), 10 * GIB);
    assert_eq!(reservation(8 * GIB, Some(1), 0), 10 * GIB);
    assert_eq!(reservation(8 * GIB, Some(12 * GIB), 0), 12 * GIB);
    assert_eq!(reservation(8 * GIB, None, 19 * GIB), 19 * GIB);
}

#[test]
fn a_partial_offload_splits_the_reservation_between_the_gpu_and_main_memory() {
    let gpus = [Memory::Gpu(0)];
    let half = GpuShare::Layers {
        on_gpu: 20,
        of: Some(40),
    };
    assert_eq!(
        llama_parts(20 * GIB, &gpus, half),
        vec![(Memory::Main, 10 * GIB), (Memory::Gpu(0), 10 * GIB)]
    );
    let usable = BTreeMap::from([(Memory::Main, 50 * GIB), (Memory::Gpu(0), 11 * GIB)]);
    let load = Load {
        parts: llama_parts(20 * GIB, &gpus, half),
    };
    assert_eq!(room(&usable, &[], &load, 0), Room::Load);
    let unknown = GpuShare::Layers {
        on_gpu: 20,
        of: None,
    };
    assert_eq!(
        llama_parts(20 * GIB, &gpus, unknown),
        vec![(Memory::Main, 20 * GIB), (Memory::Gpu(0), 20 * GIB)]
    );
}

fn now<'a>(usable: &'a Usable, held: &'a [Held], busy_at: &'a BTreeMap<Memory, u64>) -> Now<'a> {
    Now {
        usable,
        held,
        busy_at,
        permitted: true,
    }
}

#[test]
fn a_displaced_pin_returns_when_the_memory_permits() {
    let usable = usable(24);
    let busy = BTreeMap::new();
    let pin = Displaced::new(copy(9), main_load(10), None, 0);
    let full = [on_main(1, 20, OnServer::Active, 0)];
    assert_eq!(
        next(&now(&usable, &full, &busy), std::slice::from_ref(&pin), 10),
        None
    );
    let after = [on_main(1, 8, OnServer::Ready, 0)];
    assert_eq!(
        next(&now(&usable, &after, &busy), &[pin], 10),
        Some(copy(9))
    );
}

#[test]
fn a_return_does_not_hold_back_a_load_that_waits_or_run_while_the_switch_is_off() {
    let usable = usable(24);
    let pin = Displaced::new(copy(9), main_load(10), None, 0);
    let busy = BTreeMap::from([(Memory::Main, 990)]);
    assert_eq!(
        next(&now(&usable, &[], &busy), std::slice::from_ref(&pin), 1000),
        None
    );
    let loading = [on_main(1, 4, OnServer::Loading, 0)];
    let quiet = BTreeMap::new();
    assert_eq!(
        next(
            &now(&usable, &loading, &quiet),
            std::slice::from_ref(&pin),
            1000
        ),
        None
    );
    let mut off = now(&usable, &[], &quiet);
    off.permitted = false;
    assert_eq!(next(&off, &[pin], 1000), None);
}

#[test]
fn a_pin_that_is_displaced_again_soon_waits_longer_each_time_and_never_loops() {
    let first = Displaced::new(copy(9), main_load(10), None, 0);
    assert_eq!(first.interval, FIRST_INTERVAL);
    let mut last = (0, FIRST_INTERVAL);
    let mut intervals = Vec::new();
    for round in 1..=8 {
        let returned_at = round * 100_000;
        let again = Displaced::new(
            copy(9),
            main_load(10),
            Some((returned_at, last.1)),
            returned_at + 60,
        );
        intervals.push(again.interval);
        last = (returned_at, again.interval);
    }
    assert_eq!(intervals[0], 2 * FIRST_INTERVAL);
    assert_eq!(*intervals.last().unwrap_or(&0), LONGEST_INTERVAL);
    let calm = Displaced::new(
        copy(9),
        main_load(10),
        Some((0, LONGEST_INTERVAL)),
        2 * FIRST_INTERVAL,
    );
    assert_eq!(calm.interval, FIRST_INTERVAL);
    let usable = usable(24);
    let quiet = BTreeMap::new();
    let early = Displaced::new(copy(9), main_load(10), Some((1000, FIRST_INTERVAL)), 1100);
    assert_eq!(
        next(
            &now(&usable, &[], &quiet),
            std::slice::from_ref(&early),
            1100
        ),
        None
    );
    assert_eq!(
        next(
            &now(&usable, &[], &quiet),
            std::slice::from_ref(&early),
            1000 + early.interval
        ),
        Some(copy(9))
    );
}
