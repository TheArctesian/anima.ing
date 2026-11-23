//! The daemon's buffer: points wait here until the hub accepts them; no log holds a token or a body (MET-013, MET-014, LOG-007).

pub mod accounts;
pub mod pacing;
pub mod renewal;
mod renewing;
#[cfg(all(test, target_os = "linux"))]
mod tests;
pub mod unstored;
pub mod usage;

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::hardware::Hardware;
use daemon_core::points::{BATCH_POINTS, Backlog, Point};
use daemon_core::wire::{
    Announcements, Batch, BatchBattery, BatchDrive, BatchGpu, BatchInventory, PushAccepted,
    RefusalCode,
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
}

pub struct Pusher {
    hub: HubClient,
    device: Uuid,
    credential: Credential,
    store: TokenStore,
    backlog: Backlog,
    announced: Announcements,
    usage: UsageBacklog,
    backoff: Backoff,
    limit: usize,
    stopped_until: Option<DateTime<Utc>>,
    renewal_tried: Option<DateTime<Utc>>,
    spread: fn() -> f64,
    keys: Option<Keeper>,
    /// A renewed token whose save waits in the Keychain; not used until stored.
    unstored: Option<Unstored>,
    marks: Option<Home>,
    marked: bool,
}

impl Pusher {
    #[must_use]
    pub fn new(hub: HubClient, device: Uuid, credential: Credential, store: TokenStore) -> Self {
        Self {
            hub,
            device,
            credential,
            store,
            backlog: Backlog::new(),
            announced: Announcements::default(),
            usage: UsageBacklog::default(),
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

    /// Puts [`MARK`](crate::keychain_wait::MARK) in `home` while a Keychain call waits for the person (R-322).
    #[must_use]
    pub fn with_marks(self, home: Home) -> Self {
        Self {
            marks: Some(home),
            ..self
        }
    }

    /// Each batch says what the key cache holds; each answer that names the keys updates it (K-E5 Section 2.1).
    #[must_use]
    pub fn with_keys(self, keeper: Keeper) -> Self {
        Self {
            keys: Some(keeper),
            ..self
        }
    }

    #[must_use]
    pub fn with_spread(self, spread: fn() -> f64) -> Self {
        Self { spread, ..self }
    }

    pub fn accept(&mut self, event: Event) {
        match event {
            Event::Point(point) => self.backlog.push(point),
            Event::Gpus(gpus) => self.announced.gpus = Some(gpus),
            Event::Drives(drives) => self.announced.drives = Some(drives),
            Event::Batteries(batteries) => self.announced.batteries = Some(batteries),
            Event::Inventory(inventory) => self.announced.inventory = Some(inventory),
            Event::Hardware(hardware) => self.announced.hardware = Some(hardware),
            Event::AgentUsage(report) => self.usage.take(report),
        }
    }

    /// Whether pushing is stopped for a refused token (K8 Section 6).
    #[must_use]
    pub const fn is_stopped(&self) -> bool {
        self.stopped_until.is_some()
    }

    /// One round: renew when due, drop stale points, send batches oldest first; returns the wait before the next.
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
                return RHYTHM;
            }
            match self.send(waiting.min(self.limit), now).await {
                Round::Sent => {}
                Round::Wait(wait) => return wait,
            }
        }
    }

    async fn send(&mut self, count: usize, now: DateTime<Utc>) -> Round {
        let mut count = count;
        loop {
            let outgoing = self.backlog.batch().first(count);
            let Some(receipt) = outgoing.receipt() else {
                return Round::Sent;
            };
            let announced = Announcements {
                agent_usage: self.usage.outgoing(),
                ..self.announced.clone()
            };
            let mut batch =
                Batch::new(self.device, outgoing.points(), announced).from_daemon(VERSION);
            if let Some(keys) = &self.keys {
                batch = batch.with_keys(keys.held());
            }
            let answer = self.hub.push(&self.credential.token, &batch).await;
            match pacing::action(&answer) {
                Action::Remove => {
                    self.backlog.remove_through(receipt);
                    self.announced = Announcements::default();
                    self.usage.done();
                    self.limit = pacing::grown(count);
                    self.recovered();
                    if let Answer::Accepted(Some(accepted)) = answer {
                        self.usage.take_salt(accepted.account_salt.as_deref());
                        self.follow_keys(accepted, now).await;
                    }
                    return Round::Sent;
                }
                Action::Drop(code) => {
                    report::batch_dropped(code, count);
                    self.backlog.remove_through(receipt);
                    // A refused list would ride along and sink every later batch; the hub keeps what it has (K8 Section 4).
                    if code == RefusalCode::Malformed {
                        self.announced = Announcements::default();
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

    /// A refused token also deletes the whole key cache (R-147).
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

    /// Takes a changed token from the store, never one bound to another hub, which it would send to this one.
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

/// The middle when the OS source fails, since a retry wait need not be secret.
fn os_spread() -> f64 {
    let mut bytes = [0_u8; 4];
    match getrandom::fill(&mut bytes) {
        Ok(()) => f64::from(u32::from_le_bytes(bytes)) / f64::from(u32::MAX),
        Err(_) => 0.5,
    }
}

enum Round {
    Sent,
    Wait(Duration),
}
