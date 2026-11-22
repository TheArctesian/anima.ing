//! The key cache at work beside the pusher: it says what it holds, follows the hub's current
//! versions, fetches what changed, and deletes itself when the rules say so (K-E5 Section 2.1).
//! The decisions are in [`cache`](super::cache) and `pending`; this is the
//! I/O edge: the store and the hub. Each store call runs off the async loop (R-322).

use chrono::{DateTime, Utc};
use daemon_core::wire::{DaemonToken, KeyFetch, KeyVersion, KeysCurrent, RefusalCode};
use uuid::Uuid;

use super::cache::KeyCache;
use super::pending::Pending;
use super::store::{KeyStore, KeyStoreError};
use crate::hub::HubClient;
use crate::hub::answer::{Answer, Cause};
use crate::keychain_wait;

/// The cache and where it is kept.
pub struct Keeper {
    cache: KeyCache,
    store: KeyStore,
    /// What the store owes after the Keychain waited (R-322).
    pending: Pending,
}

/// What [`Keeper::follow`] needs of the pusher: its hub, its token and its device.
pub struct Asking<'a> {
    pub hub: &'a HubClient,
    pub token: &'a DaemonToken,
    pub device: Uuid,
}

impl Keeper {
    /// The keeper of the cache in `store`. A cache that cannot be read is deleted, and the
    /// daemon starts with none: it fetches its keys again. A read that waits for the person in
    /// the Keychain deletes nothing: the delete would wait the same way (R-322).
    #[must_use]
    pub fn open(store: KeyStore) -> Self {
        let cache = store.load().unwrap_or_else(|error| {
            report::not_read(&error);
            if matches!(error, KeyStoreError::KeychainWaits) {
                return KeyCache::default();
            }
            if let Err(error) = store.forget() {
                report::not_deleted(&error);
            }
            KeyCache::default()
        });
        Self {
            cache,
            store,
            pending: Pending::default(),
        }
    }

    /// The versions the cache holds: the `held` of the next batch.
    #[must_use]
    pub fn held(&self) -> Vec<KeyVersion> {
        self.cache.held()
    }

    /// Whether a call of the store waits for the person in the Keychain (R-322).
    #[must_use]
    pub const fn waits(&self) -> bool {
        self.pending.waits()
    }

    /// Each round at `now`: deletes the whole cache when no answer with `keys` came in the last
    /// 7 days (R-147), and tries again what the store owes after the Keychain waited.
    pub async fn tend(&mut self, now: DateTime<Utc>) {
        if self.cache.is_stale(now) {
            report::expired();
            self.forget().await;
        } else if self.pending.delete_owed() {
            self.delete().await;
        } else if self.pending.waits() {
            self.save(now).await;
        }
    }

    /// Deletes the whole cache: after a refused token, and when the daemon leaves the machine.
    /// A delete is always tried, also while saves wait, as it removes key values; one that
    /// waits is tried again at each round (R-147).
    pub async fn forget(&mut self) {
        self.cache.forget();
        self.delete().await;
    }

    /// Follows the hub's `current` versions told at `now`: drops what is no longer current at
    /// once, stores that, then fetches each current version it lacks and stores again. A
    /// fetch that fails is tried at the next answer.
    pub async fn follow(&mut self, asking: Asking<'_>, current: &KeysCurrent, now: DateTime<Utc>) {
        let wanted = self.cache.apply(&current.current, now);
        self.save(now).await;
        if wanted.is_empty() {
            return;
        }
        for one in &wanted {
            let asked = KeyFetch {
                device: asking.device,
                name: one.name.clone(),
            };
            match asking.hub.fetch_key(asking.token, &asked).await {
                Answer::Accepted(fetched) => {
                    if !self.cache.keep(fetched, &wanted) {
                        report::fetch_failed(Cause::Unreadable);
                    }
                }
                // The hub refuses the token: the whole cache goes, as after a refused push
                // (K-E5 Section 2.1).
                Answer::Refused(RefusalCode::TokenRefused) => {
                    report::fetch_refused(RefusalCode::TokenRefused);
                    self.forget().await;
                    return;
                }
                Answer::Refused(code) => report::fetch_refused(code),
                Answer::Failed(cause) => report::fetch_failed(cause),
            }
        }
        self.save(now).await;
    }

    /// Deletes the stored cache; logs a first wait and any other failure.
    async fn delete(&mut self) {
        let store = self.store.clone();
        let answer = keychain_wait::off_loop(move || store.forget()).await;
        if self.pending.deleted(&answer) {
            report::delete_waits();
        }
        if let Err(error) = &answer
            && !matches!(error, KeyStoreError::KeychainWaits)
        {
            report::not_deleted(error);
        }
    }

    /// Stores the cache at `now`, unless an earlier save waited in the Keychain and its next
    /// try is later: the cache is then kept in memory only.
    async fn save(&mut self, now: DateTime<Utc>) {
        if !self.pending.save_due(now) {
            return;
        }
        let record = KeyStore::record(&self.cache);
        let store = self.store.clone();
        let answer =
            keychain_wait::off_loop(move || store.keep(record.as_deref().map(Vec::as_slice))).await;
        if self.pending.saved(&answer, now) {
            report::saves_wait();
        }
        if let Err(error) = &answer
            && !matches!(error, KeyStoreError::KeychainWaits)
        {
            report::not_stored(error);
        }
    }
}

/// The keeper's log lines: by code or cause, never a key name or a value (LOG-007).
mod report {
    use daemon_core::wire::RefusalCode;

    use super::{Cause, KeyStoreError};

    pub fn not_read(error: &KeyStoreError) {
        tracing::error!(%error, "the key cache could not be read; it is deleted");
    }

    pub fn not_stored(error: &KeyStoreError) {
        tracing::error!(%error, "the key cache could not be stored");
    }

    pub fn saves_wait() {
        tracing::warn!(
            "the key cache could not be stored: this computer's daemon needs you to allow it in the Keychain; the cache is kept in memory, and stored again every 30 s, after 10 minutes every 5 minutes"
        );
    }

    pub fn delete_waits() {
        tracing::warn!(
            "the key cache could not be deleted: this computer's daemon needs you to allow it in the Keychain; the delete is tried again at each push round"
        );
    }

    pub fn not_deleted(error: &KeyStoreError) {
        tracing::error!(%error, "the key cache could not be deleted");
    }

    pub fn expired() {
        tracing::warn!("no answer of the hub named the keys for 7 days; the key cache is deleted");
    }

    pub fn fetch_refused(code: RefusalCode) {
        tracing::warn!(code = code.as_str(), "the hub refused a key fetch");
    }

    pub fn fetch_failed(cause: Cause) {
        tracing::warn!(%cause, "a key fetch failed; trying again at the next push");
    }
}
