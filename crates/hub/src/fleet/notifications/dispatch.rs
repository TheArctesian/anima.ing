//! The dispatcher: one task that sends queued notifications and retries failed ones
//! (D-16, NOT-001, NOT-002, P3-003).
//!
//! A claim moves `next_attempt_at` [`LEASE`] ahead with `FOR UPDATE SKIP LOCKED`, so dispatchers
//! take disjoint rows. Delivery is at least once: a row is sent again only when its lease ends
//! before its outcome is written. Every outcome write matches the lease end its claim set, so a
//! claimer whose lease ended changes nothing. An answered 2xx or a mail accepted after DATA
//! counts as sent and is never retried (S-A, S-B).

mod claimed;
mod failure;
mod outgoing;
mod retry;
mod sender;
#[cfg(test)]
pub(super) mod tests;

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use tokio::{task::JoinSet, time::MissedTickBehavior};

use super::{Error, Outbox, egress::Egress, store};
use crate::{
    clock::Clock,
    fleet::{
        credentials::Vault,
        events::{self, Bus},
        targets::AddressPolicy,
    },
};

use claimed::Finished;
use failure::Failure;
pub(super) use failure::delivery;
pub(super) use sender::Sender;

const TICK: Duration = Duration::from_secs(1);
/// Far longer than a send (8 s) and its two database round trips.
const LEASE: TimeDelta = TimeDelta::minutes(5);
const BATCH: i64 = 16;

/// The vault that opens channel secrets, and the address policy every notification connection
/// obeys (K3, P3-003).
pub struct Sending {
    pub vault: Vault,
    pub policy: AddressPolicy,
}

/// The row comes back when its lease ends.
#[derive(Debug, thiserror::Error)]
enum Fault {
    #[error("the notification store failed")]
    Store(#[from] Error),
    #[error("the Warning event could not be written")]
    Events(#[from] events::Error),
}

impl Fault {
    /// The fixed word the process log carries (LOG-007).
    fn word(&self) -> &'static str {
        match self {
            Self::Store(error) => error.fault(),
            Self::Events(error) => error.fault(),
        }
    }
}

/// A failed read or write logs a fixed word, and the row comes back after its lease.
#[must_use]
pub fn spawn_dispatcher(
    db: PgPool,
    bus: Bus,
    clock: Arc<dyn Clock>,
    sending: Sending,
) -> tokio::task::JoinHandle<()> {
    let sender = Sender::new(sending.vault, Egress::new(sending.policy));
    let dispatcher = Arc::new(Dispatcher { db, bus, sender });
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            dispatcher.tick(clock.now()).await;
        }
    })
}

/// Each row of a batch runs in its own task; none holds a lock.
struct Dispatcher {
    db: PgPool,
    bus: Bus,
    sender: Sender,
}

impl Dispatcher {
    async fn tick(self: &Arc<Self>, now: DateTime<Utc>) -> usize {
        let defaults = match store::installation_settings(&self.db).await {
            Ok(settings) => Arc::new(settings),
            Err(error) => {
                warn_fault(None, &Fault::from(error));
                return 0;
            }
        };
        let claimed = match store::claim_due(&self.db, now, LEASE, BATCH).await {
            Ok(claimed) => claimed,
            Err(error) => {
                warn_fault(None, &Fault::from(error));
                return 0;
            }
        };
        let count = claimed.len();
        let mut sends = JoinSet::new();
        for row in claimed {
            let (dispatcher, defaults) = (Arc::clone(self), Arc::clone(&defaults));
            sends.spawn(async move {
                let finished = dispatcher.finish(&row, &defaults, now).await;
                (row, finished)
            });
        }
        while let Some(joined) = sends.join_next().await {
            match joined {
                Ok((row, finished)) => log_finished(&row, &finished),
                Err(error) => tracing::error!(
                    domain = "notifications",
                    panicked = error.is_panic(),
                    "a notification send ended abnormally; its row comes back after its lease"
                ),
            }
        }
        count
    }
}

/// Logs the channel id, the reason word and the code only (LOG-007).
fn log_finished(row: &Outbox, finished: &Result<Finished, Fault>) {
    match finished {
        Ok(Finished::Sent | Finished::Ended) => {}
        Ok(Finished::OutOfTries) => tracing::info!(
            domain = "notifications",
            channel_id = %row.channel,
            attempts = row.attempts,
            "a notification ran out of tries with no outcome recorded"
        ),
        Ok(Finished::Failed(failure)) => log_failed(row, *failure),
        Err(fault) => warn_fault(Some(row), fault),
    }
}

fn log_failed(row: &Outbox, failure: Failure) {
    tracing::info!(
        domain = "notifications",
        channel_id = %row.channel,
        reason = failure.reason.as_str(),
        code = failure.code,
        attempts = row.attempts,
        "a notification was not delivered"
    );
}

fn warn_fault(row: Option<&Outbox>, fault: &Fault) {
    tracing::warn!(
        domain = "notifications",
        channel_id = row.map(|row| tracing::field::display(row.channel)),
        fault = fault.word(),
        "a notification could not be dispatched; it is tried again after its lease"
    );
}
