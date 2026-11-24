//! The one attempt the hub holds, in memory only, and the broadcast of its changes (K-E1
//! Section 7a). A hub restart drops it.
//!
//! The lock is a std mutex, never held across an `await`; a change is published after it is
//! released. Work that holds the attempt between two phases across an `await` holds a
//! [`Settle`], so the attempt leaves the phase even when that work is dropped or panics.

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::sync::{Notify, broadcast};
use uuid::Uuid;

use super::attempt::{Attempt, Phase, Refusal, Snapshot};
use crate::clock::Clock;

const CAPACITY: usize = 32;

#[derive(Clone)]
pub struct Attempts {
    held: Arc<Mutex<Option<Attempt>>>,
    sender: broadcast::Sender<Snapshot>,
    wake: Arc<Notify>,
    poll: Duration,
    ending_wait: Duration,
}

impl Attempts {
    #[must_use]
    pub fn new(poll: Duration, ending_wait: Duration) -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self {
            held: Arc::default(),
            sender,
            wake: Arc::default(),
            poll,
            ending_wait,
        }
    }

    fn with<T>(&self, now: DateTime<Utc>, f: impl FnOnce(&mut Option<Attempt>) -> T) -> T {
        let (result, before, after) = {
            let mut slot = self.held.lock().unwrap_or_else(PoisonError::into_inner);
            let before = slot.as_ref().map(Attempt::snapshot);
            if let Some(held) = slot.as_mut() {
                held.expire(now);
            }
            if slot.as_ref().is_some_and(|held| !held.readable(now)) {
                *slot = None;
            }
            let result = f(&mut slot);
            (result, before, slot.as_ref().map(Attempt::snapshot))
        };
        if let Some(after) = after.filter(|after| before.as_ref() != Some(after)) {
            // `send` fails only when nobody is subscribed: there is nobody to tell.
            let _ = self.sender.send(after);
        }
        result
    }

    /// # Errors
    /// `AppWaiting` while an App waits for Install.
    pub fn start(&self, attempt: Attempt, now: DateTime<Utc>) -> Result<Snapshot, Refusal> {
        self.with(now, |slot| {
            if slot
                .as_ref()
                .is_some_and(|held| !held.ended() && held.holds_app())
            {
                return Err(Refusal::AppWaiting);
            }
            let snapshot = attempt.snapshot();
            *slot = Some(attempt);
            Ok(snapshot)
        })
    }

    pub fn forget(&self, id: Uuid, now: DateTime<Utc>) {
        self.with(now, |slot| {
            if slot.as_ref().is_some_and(|held| held.id() == id) {
                *slot = None;
            }
        });
    }

    /// Open, or ended in the last 10 minutes.
    #[must_use]
    pub fn current(&self, now: DateTime<Utc>) -> Option<Snapshot> {
        self.with(now, |slot| slot.as_ref().map(Attempt::snapshot))
    }

    /// # Errors
    /// `NotFound` when no attempt is held, or another one; the refusal of `step` otherwise.
    pub fn step<T>(
        &self,
        id: Option<Uuid>,
        now: DateTime<Utc>,
        step: impl FnOnce(&mut Attempt) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        self.with(now, |slot| match slot.as_mut() {
            Some(held) if id.is_none_or(|id| id == held.id()) => step(held),
            _ => Err(Refusal::NotFound),
        })
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Snapshot> {
        self.sender.subscribe()
    }

    pub fn wake(&self) {
        self.wake.notify_one();
    }

    pub async fn woken(&self) {
        self.wake.notified().await;
    }

    #[must_use]
    pub const fn poll(&self) -> Duration {
        self.poll
    }

    /// `None` when the attempt has not ended within `ending_wait` or is no longer held.
    pub async fn ended_within(&self, id: Uuid, clock: &dyn Clock) -> Option<Phase> {
        // Subscribed before the first read, so no change between the two is missed.
        let mut changes = self.subscribe();
        let mut seen = self.current(clock.now());
        let ended = async move {
            loop {
                let held = seen.filter(|held| held.id == id)?;
                if matches!(
                    held.phase,
                    Phase::Connected | Phase::Failed | Phase::Cancelled
                ) {
                    return Some(held.phase);
                }
                seen = match changes.recv().await {
                    Ok(change) => Some(change),
                    Err(broadcast::error::RecvError::Lagged(_)) => self.current(clock.now()),
                    // The sender lives in `self`: the feed cannot close while this waits.
                    Err(broadcast::error::RecvError::Closed) => return None,
                };
            }
        };
        tokio::time::timeout(self.ending_wait, ended)
            .await
            .ok()
            .flatten()
    }
}

/// Runs its step under the lock when dropped, whether the work ended, was dropped or panicked.
pub struct Settle {
    attempts: Attempts,
    id: Uuid,
    clock: Arc<dyn Clock>,
    step: Option<SettleStep>,
}

type SettleStep = Box<dyn FnOnce(&mut Attempt, DateTime<Utc>) + Send>;

impl Settle {
    #[must_use]
    pub fn new(
        attempts: Attempts,
        id: Uuid,
        clock: Arc<dyn Clock>,
        step: impl FnOnce(&mut Attempt, DateTime<Utc>) + Send + 'static,
    ) -> Self {
        Self {
            attempts,
            id,
            clock,
            step: Some(Box::new(step)),
        }
    }

    pub fn instead(&mut self, step: impl FnOnce(&mut Attempt, DateTime<Utc>) + Send + 'static) {
        self.step = Some(Box::new(step));
    }
}

impl Drop for Settle {
    fn drop(&mut self) {
        if let Some(step) = self.step.take() {
            let now = self.clock.now();
            // `NotFound` means the attempt was replaced: nothing is left to settle.
            let _ = self.attempts.step(Some(self.id), now, |held| {
                step(held, now);
                Ok(())
            });
        }
    }
}
