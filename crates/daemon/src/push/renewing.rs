//! The pusher's renewal of its token (MET-013, K8 Section 5): it asks the hub when
//! [`renewal::due`] says so, stores the new token, and only then uses it. A save that waits
//! for the person in the Keychain is carried out by [`unstored`] (R-322).

use chrono::{DateTime, Utc};
use daemon_core::wire::Renewed;

use super::unstored::{self, Stored};
use super::{Pusher, renewal};
use crate::hub::answer::Answer;
use crate::keychain_wait;
use crate::report;
use crate::token_store::Credential;

impl Pusher {
    /// Renews the token inside its last 30 days (K8 Section 5): the new token is stored first,
    /// and only then used. If it cannot be stored the current one stays. While the save of a
    /// renewed token waits in the Keychain, only that save is tried again ([`unstored`]).
    pub(super) async fn renew_if_due(&mut self, now: DateTime<Utc>) {
        if let Some(unstored) = self.unstored.take() {
            let store = &self.store;
            let stored = unstored.retry(now, |next| store.save(next));
            self.settle(stored);
            return;
        }
        if !renewal::due(self.credential.expires_at, now, self.renewal_tried) {
            return;
        }
        self.renewal_tried = Some(now);
        match self.hub.renew(&self.credential.token).await {
            Answer::Accepted(renewed) => self.keep_renewed(renewed, now),
            Answer::Refused(code) => report::renewal_refused(code),
            Answer::Failed(cause) => report::renewal_failed(cause),
        }
    }

    /// Stores the renewed token and only then uses it; on a failure the current one stays.
    fn keep_renewed(&mut self, renewed: Renewed, now: DateTime<Utc>) {
        let hub = self.credential.hub.clone();
        let next = Credential::received(renewed.token, renewed.expires_at, hub, now);
        let store = &self.store;
        let stored = unstored::store(next, now, |next| store.save(next));
        self.settle(stored);
    }

    /// Acts on a try to store a renewed token: uses it once stored, keeps it unused while its
    /// save waits, drops it on any other failure.
    fn settle(&mut self, stored: Stored) {
        match stored {
            Stored::Yes(next) => {
                self.credential = next;
                self.mark(false);
                report::renewed();
            }
            Stored::Waits(unstored) => {
                if unstored.first_wait() {
                    report::renewal_waits();
                    self.mark(true);
                }
                self.unstored = Some(unstored);
            }
            Stored::No(error) => {
                self.mark(false);
                report::renewal_not_stored(&error);
            }
        }
    }

    /// Puts or takes away the mark of a wait in the Keychain, when this pusher keeps one.
    pub(super) fn mark(&self, waits: bool) {
        if let Some(home) = &self.marks
            && let Err(error) = keychain_wait::mark(home, waits)
        {
            report::keychain_mark_refused(&error);
        }
    }
}
