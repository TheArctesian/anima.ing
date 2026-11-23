//! Reads each SSH metrics device over the one SSH link with fixed read-only commands
//! (HUB-004, MET-001, MET-002, P2-003, D-2, D-9 to D-11).

mod collection;
mod detection;
mod failures;
mod fan_out;
mod gpus;
pub(super) mod os_release;
mod outputs;
mod plan;
mod round;
#[cfg(test)]
mod tests;
mod uname;

use std::{sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::{task::JoinHandle, time::MissedTickBehavior};

use super::Feed;
use crate::{
    clock::Clock,
    fleet::{credentials, events::Bus, targets::AddressPolicy},
};

use round::Collector;

/// The time between rounds (MET-001).
const ROUND: Duration = Duration::from_secs(30);
/// The most collections that run at the same time (D-11).
const AT_ONCE: usize = 8;
/// The longest one device's link and commands take together (D-11).
const DEADLINE: Duration = Duration::from_secs(20);
/// Inside [`DEADLINE`], so a slow GPU tool ends the GPU reads, never the whole collection.
const GPU_WINDOW: Duration = Duration::from_secs(18);

/// The credential vault, and the address policy every link obeys (P2-004).
pub struct Collecting {
    pub vault: credentials::Vault,
    pub policy: AddressPolicy,
}

struct Context {
    db: PgPool,
    feed: Feed,
    bus: Bus,
    clock: Arc<dyn Clock>,
    collecting: Collecting,
    gpu_window: Duration,
}

/// Missed ticks are skipped, so rounds never stack; aborting the handle aborts collections in flight.
/// A failed link, read or write is logged and the device is tried again next round.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the hub contract (K3) fixes the signature of spawn_collector"
)]
pub fn spawn_collector(
    db: PgPool,
    feed: Feed,
    bus: Bus,
    clock: Arc<dyn Clock>,
    collecting: Collecting,
) -> JoinHandle<()> {
    let mut collector = Collector::new(Context {
        db,
        feed,
        bus,
        clock,
        collecting,
        gpu_window: GPU_WINDOW,
    });
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(ROUND);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            collector.round().await;
        }
    })
}
