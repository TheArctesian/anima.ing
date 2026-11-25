//! The daemon's buffer and its pushes: points wait here until the hub accepts them (MET-013,
//! MET-014, K8 Sections 5 to 7). The decisions are in [`pacing`](crate::push::pacing),
//! [`renewal`](crate::push::renewal) and [`answer`](crate::hub::answer); this module carries them out and
//! logs the changes of state (never a token or a body, LOG-007).

pub mod accounts;
mod batch;
mod inventory;
pub mod pacing;
pub mod renewal;
mod renewing;
#[cfg(all(test, target_os = "linux"))]
mod tests;
#[cfg(unix)]
mod thread_summaries;
pub mod unstored;
pub mod usage;

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::hardware::Hardware;
use daemon_core::points::{BATCH_POINTS, Backlog, Point};
use daemon_core::wire::{
    Announcements, BatchBattery, BatchDrive, BatchGpu, BatchInventory, PushAccepted, RefusalCode,
};
use uuid::Uuid;

use crate::config::home::Home;
use crate::hub::HubClient;
use crate::hub::answer::Answer;
use crate::keychain_wait;
use crate::keys::keeper::{Asking, Keeper};

use crate::report;
use crate::sampling::agent_usage::UsageReport;
use crate::token_store::{Credential, TokenStore};
use pacing::{Action, Backoff, RHYTHM};
use unstored::Unstored;
use usage::UsageBacklog;

/// The version of this daemon, sent with each batch (R-122); `build.rs` sets it for a release.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the sampling side hands the pusher.
pub enum Event {
    Point(Point),
    /// The GPUs found at start or at a discovery; sent in the next batch (K8 Section 4).
    Gpus(Vec<BatchGpu>),
    /// The drives read at start or after a change; sent in the next batch (MET-027).
    Drives(Vec<BatchDrive>),
    /// The batteries read at start or after one came, went or changed its state (MET-032).
    Batteries(Vec<BatchBattery>),
    /// The tool inventory probed at start or after a change; sent in the next batch (CAP-027).
    Inventory(BatchInventory),
    /// The hardware values read with the inventory; sent in the next batch (CAP-032).
    Hardware(Hardware),
    /// The daemon's agent usage state and the session days that changed (AIS-019).
    AgentUsage(UsageReport),
    #[cfg(unix)]
    ThreadSummaries(
        Option<(
            daemon_core::wire::ThreadSummaries,
            crate::config::agent_usage::UsageSettings,
            std::collections::BTreeSet<Uuid>,
        )>,
    ),
}

/// Everything the pusher keeps between pushes.
pub struct Pusher {
    hub: std::sync::Arc<HubClient>,
    device: Uuid,
    credential: Credential,
    store: TokenStore,
    backlog: Backlog,
    /// The GPUs, drives, batteries and inventory waiting to ride along with the next batch.
    announced: Announcements,
    inventory_deferred: bool,
    /// The agent usage waiting for the hub, a part of it in each batch.
    usage: UsageBacklog,
    #[cfg(unix)]
    thread_summaries: thread_summaries::Pending,
    backoff: Backoff,
    /// The largest batch to try; smaller after `TOO_LARGE`.
    limit: usize,
    /// When a refused token is tried again; `Some` while pushing is stopped.
    stopped_until: Option<DateTime<Utc>>,
    renewal_tried: Option<DateTime<Utc>>,
    /// Where the spread of each retry wait comes from: a number from 0 to 1.
    spread: fn() -> f64,
    /// The operational key cache; `None` for a pusher that keeps none and sends no `keys`.
    keys: Option<Keeper>,
    /// A renewed token whose save waits for the person in the Keychain; not used until stored.
    unstored: Option<Unstored>,
    /// The daemon's folder, where [`MARK`](crate::keychain_wait::MARK) tells the app that a
    /// call of the Keychain waits.
    marks: Option<Home>,
    /// Whether this pusher put the mark.
    marked: bool,
}

impl Pusher {
    #[must_use]
    pub fn new(hub: HubClient, device: Uuid, credential: Credential, store: TokenStore) -> Self {
        Self {
            hub: std::sync::Arc::new(hub),
            device,
            credential,
            store,
            backlog: Backlog::new(),
            announced: Announcements::default(),
            inventory_deferred: false,
            usage: UsageBacklog::default(),
            #[cfg(unix)]
            thread_summaries: thread_summaries::Pending::default(),
            backoff: Backoff::default(),
            limit: BATCH_POINTS,
            stopped_until: None,
            renewal_tried: None,
            spread: os_spread,
            keys: None,
            unstored: None,
            marks: None,
            marked: false,
        }
    }

    #[cfg(unix)]
    pub(crate) fn thread_sync_target(&self) -> crate::runtime::thread_sync::Target {
        crate::runtime::thread_sync::Target {
            hub: self.hub.clone(),
            device: self.device,
            origin: self.credential.hub.clone(),
            tokens: self.store.clone(),
        }
    }

    /// The same pusher, which puts [`MARK`](crate::keychain_wait::MARK) in `home` while the
    /// save of a renewed token or a call of the key store waits for the person in the
    /// Keychain (R-322).
    #[must_use]
    pub fn with_marks(self, home: Home) -> Self {
        Self {
            marks: Some(home),
            ..self
        }
    }

    /// The same pusher with the operational key cache of `keeper`: each batch says what it
    /// holds, and each answer that names the keys updates it (K-E5 Section 2.1).
    #[must_use]
    pub fn with_keys(self, keeper: Keeper) -> Self {
        Self {
            keys: Some(keeper),
            ..self
        }
    }

    /// The same pusher with its retry waits spread by `spread` instead of the OS random source.
    #[must_use]
    pub fn with_spread(self, spread: fn() -> f64) -> Self {
        Self { spread, ..self }
    }

    /// Takes what sampling produced.
    pub fn accept(&mut self, event: Event) {
        match event {
            Event::Point(point) => self.backlog.push(point),
            Event::Gpus(gpus) => self.announced.gpus = Some(gpus),
            Event::Drives(drives) => self.announced.drives = Some(drives),
            Event::Batteries(batteries) => self.announced.batteries = Some(batteries),
            Event::Inventory(inventory) => self.announced.inventory = Some(inventory),
            Event::Hardware(hardware) => self.announced.hardware = Some(hardware),
            Event::AgentUsage(report) => self.usage.take(report),
            #[cfg(unix)]
            Event::ThreadSummaries(batch) => self.thread_summaries.replace(batch),
        }
    }

    /// Whether pushing is stopped for a refused token (K8 Section 6).
    #[must_use]
    pub const fn is_stopped(&self) -> bool {
        self.stopped_until.is_some()
    }

    /// One round at `now`: renew when due, drop stale points, then send batches oldest first
    /// until none is left or one fails. Returns how long to wait before the next round.
    pub async fn push_due(&mut self, now: DateTime<Utc>) -> Duration {
        if let Some(keys) = &mut self.keys {
            keys.tend(now).await;
        }
        if self.stopped_until.is_none() {
            self.renew_if_due(now).await;
        }
        let adopted = self.stopped_until.is_some() && self.adopt_changed_token().await;
        self.show_waits();
        if let Some(retry_at) = self.stopped_until
            && !adopted
            && now < retry_at
        {
            return RHYTHM;
        }
        loop {
            self.backlog.drop_stale(now);
            let waiting = self.backlog.batch().points().len();
            if waiting == 0 {
                return self.send_inventory(now).await;
            }
            match self.send(waiting.min(self.limit), now).await {
                Round::Sent => {}
                Round::Wait(wait) => return wait,
            }
        }
    }

    /// Sends one batch of at most `count` points and acts on the answer.
    async fn send(&mut self, count: usize, now: DateTime<Utc>) -> Round {
        let mut count = count;
        loop {
            let outgoing = self.backlog.batch().first(count);
            let Some(receipt) = outgoing.receipt() else {
                return Round::Sent;
            };
            let mut batch = self.batch(count);
            let deferred_inventory = inventory::defer(&mut batch);
            self.inventory_deferred = deferred_inventory;
            let answer = if inventory::fits(&batch) {
                self.hub.push(&self.credential.token, &batch).await
            } else {
                Answer::Refused(RefusalCode::TooLarge)
            };
            match pacing::action(&answer) {
                Action::Remove => {
                    self.backlog.remove_through(receipt);
                    self.clear_announcements(deferred_inventory);
                    self.usage.done();
                    self.limit = pacing::grown(count);
                    self.recovered();
                    if let Answer::Accepted(Some(accepted)) = answer {
                        self.usage.take_salt(accepted.account_salt.as_deref());
                        #[cfg(unix)]
                        self.accept_thread_summaries(&batch, &accepted).await;
                        self.follow_keys(accepted, now).await;
                    }
                    return Round::Sent;
                }
                Action::Drop(code) => {
                    report::batch_dropped(code, count);
                    self.backlog.remove_through(receipt);
                    // A refused list would ride along again and sink every later batch until
                    // the next discovery; the hub keeps the GPUs it has (K8 Section 4). Any
                    // MALFORMED clears the lists, so a correct one in a batch refused for its
                    // points also waits for the next discovery or change.
                    if code == RefusalCode::Malformed {
                        self.clear_announcements(deferred_inventory);
                        self.usage.done();
                    }
                    return Round::Sent;
                }
                Action::Shrink => {
                    let Some(smaller) = pacing::smaller(count) else {
                        report::batch_dropped(RefusalCode::TooLarge, 1);
                        self.backlog.remove_through(receipt);
                        return Round::Sent;
                    };
                    self.limit = smaller;
                    count = smaller;
                }
                Action::Stop(code) => return Round::Wait(self.stop(code, now).await),
                Action::Wait => return Round::Wait(self.failed(&answer)),
            }
        }
    }

    /// Updates the key cache from the hub's `Accepted`, when it named the keys.
    async fn follow_keys(&mut self, accepted: PushAccepted, now: DateTime<Utc>) {
        let (Some(keys), Some(current)) = (&mut self.keys, accepted.keys) else {
            return;
        };
        let asking = Asking {
            hub: &self.hub,
            token: &self.credential.token,
            device: self.device,
        };
        keys.follow(asking, &current, now).await;
    }

    fn failed<T>(&mut self, answer: &Answer<T>) -> Duration {
        if !self.backoff.is_failing() {
            match answer {
                Answer::Failed(cause) => report::push_failed(*cause),
                _ => report::hub_cannot_store(),
            }
        }
        self.backoff.failed((self.spread)())
    }

    fn recovered(&mut self) {
        if self.backoff.is_failing() {
            report::hub_back();
        }
        if self.stopped_until.take().is_some() {
            report::token_accepted_again();
        }
        self.backoff.recovered();
    }

    /// Stops pushing for 10 minutes. Logs only when it was pushing before (K8 Section 6). A
    /// refused token also deletes the whole key cache (R-147).
    async fn stop(&mut self, code: RefusalCode, now: DateTime<Utc>) -> Duration {
        if self.stopped_until.is_none() {
            report::token_refused(code);
        }
        if code == RefusalCode::TokenRefused
            && let Some(keys) = &mut self.keys
        {
            keys.forget().await;
        }
        let wait = TimeDelta::from_std(pacing::REFUSED_RETRY).unwrap_or(TimeDelta::minutes(10));
        self.stopped_until = Some(now + wait);
        RHYTHM
    }

    /// When the token store holds another token than the one in use, issued by the same hub,
    /// takes it and resumes. A token bound to another hub is never taken: it would be sent
    /// to this one. A renewed token whose save waited goes with it: the store holds either
    /// that token, stored late, or a newer one.
    async fn adopt_changed_token(&mut self) -> bool {
        let store = self.store.clone();
        let Ok(Some(stored)) = keychain_wait::off_loop(move || store.load()).await else {
            return false;
        };
        if stored.hub != self.credential.hub || stored.token.hash() == self.credential.token.hash()
        {
            return false;
        }
        self.credential = stored;
        self.stopped_until = None;
        self.unstored = None;
        report::token_store_changed();
        true
    }
}

/// A number from 0 to 1 from the OS random source; the middle, which keeps the step, when the
/// source fails, since a retry wait need not be secret.
fn os_spread() -> f64 {
    let mut bytes = [0_u8; 4];
    match getrandom::fill(&mut bytes) {
        Ok(()) => f64::from(u32::from_le_bytes(bytes)) / f64::from(u32::MAX),
        Err(_) => 0.5,
    }
}

enum Round {
    /// A batch left the buffer; the next goes at once.
    Sent,
    Wait(Duration),
}
