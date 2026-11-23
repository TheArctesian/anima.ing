//! How fast the hub sends live requests for one account (decision 6 of the slice): GitHub advises serial requests and a second between two writes.
//! The map's lock is never held across an `await`; a request holds a semaphore permit while it is sent.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{clock::Clock, ids::SourceAccountId};

pub(super) const READS_AT_ONCE: usize = 2;
pub(super) const CHANGE_GAP: TimeDelta = TimeDelta::seconds(1);
pub(super) const QUEUE_WAIT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Busy;

struct Lanes {
    reads: Arc<Semaphore>,
    changes: Arc<Semaphore>,
    last_change: Option<DateTime<Utc>>,
}

impl Default for Lanes {
    fn default() -> Self {
        Self {
            reads: Arc::new(Semaphore::new(READS_AT_ONCE)),
            changes: Arc::new(Semaphore::new(1)),
            last_change: None,
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct Pace(Arc<Mutex<HashMap<SourceAccountId, Lanes>>>);

/// While it lives no other change of the account starts; on drop the end of the change is noted, sent or not.
pub(super) struct ChangeTurn {
    pace: Pace,
    account: SourceAccountId,
    clock: Arc<dyn Clock>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for ChangeTurn {
    fn drop(&mut self) {
        let ended = self.clock.now();
        self.pace
            .with(self.account, |lanes| lanes.last_change = Some(ended));
    }
}

pub(super) fn gap(last: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Duration {
    last.map_or(Duration::ZERO, |last| {
        (last + CHANGE_GAP - now).to_std().unwrap_or(Duration::ZERO)
    })
}

impl Pace {
    fn with<T>(&self, account: SourceAccountId, f: impl FnOnce(&mut Lanes) -> T) -> T {
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        f(map.entry(account).or_default())
    }

    pub(super) async fn read(
        &self,
        account: SourceAccountId,
    ) -> Result<OwnedSemaphorePermit, Busy> {
        let lane = self.with(account, |lanes| lanes.reads.clone());
        tokio::time::timeout(QUEUE_WAIT, lane.acquire_owned())
            .await
            .map_err(|_| Busy)?
            .map_err(|_| Busy)
    }

    pub(super) async fn change(
        &self,
        account: SourceAccountId,
        clock: Arc<dyn Clock>,
    ) -> Result<ChangeTurn, Busy> {
        let lane = self.with(account, |lanes| lanes.changes.clone());
        let permit = tokio::time::timeout(QUEUE_WAIT, lane.acquire_owned())
            .await
            .map_err(|_| Busy)?
            .map_err(|_| Busy)?;
        let last = self.with(account, |lanes| lanes.last_change);
        tokio::time::sleep(gap(last, clock.now())).await;
        Ok(ChangeTurn {
            pace: self.clone(),
            account,
            clock,
            _permit: permit,
        })
    }
}
