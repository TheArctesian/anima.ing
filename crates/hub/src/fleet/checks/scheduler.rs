//! The check scheduler (HUB-003, CHK-001, CHK-002, CHK-010).
//! The decisions are pure and live in `schedule`; `tick` holds the loop state and the I/O.

mod attempt;
mod log;
mod moved_host;
mod notices;
mod repair;
mod schedule;
#[cfg(test)]
mod tests;
mod tick;

use std::{sync::Arc, time::Duration};

use rand::{SeedableRng as _, rngs::StdRng};
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

use crate::{clock::Clock, fleet::events::Bus};

use attempt::Context;
pub use attempt::Probing;
pub(super) use attempt::in_maintenance;
use tick::{Limits, Scheduler};

const TICK: Duration = Duration::from_secs(1);
const LIMITS: Limits = Limits {
    probes: 32,
    due_per_tick: 256,
};

/// Runs until the handle is aborted, which also aborts the probes in flight; a failed read or
/// write is logged and tried again. After a sleep it goes on as after a start (HMD-047, CHK-010).
#[must_use]
pub fn spawn_scheduler(
    db: PgPool,
    bus: Bus,
    clock: Arc<dyn Clock>,
    probing: Probing,
) -> tokio::task::JoinHandle<()> {
    let mut rng = StdRng::from_rng(&mut rand::rng());
    let mut scheduler = Scheduler::new(Context::new(db, bus, probing), LIMITS);
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut idle_since = None;
        loop {
            ticker.tick().await;
            let now = clock.now();
            if schedule::slept(idle_since, now) {
                scheduler.woke(now);
            }
            scheduler.tick(now, &mut rng).await;
            idle_since = Some(clock.now());
        }
    })
}
