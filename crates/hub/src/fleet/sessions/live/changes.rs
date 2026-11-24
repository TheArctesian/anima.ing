//! The live feed of session changes (RA-034, LOG-010). A change is published only once its row
//! is committed, and never carries session content, a token or the gateway's connection ID
//! (RA-008, LOG-007).

use tokio::sync::broadcast;

use super::Live;
use crate::fleet::sessions::model::{Session, SessionChange};

const CAPACITY: usize = 256;

/// A publisher never waits for a subscriber; one that lags skips the oldest changes.
#[derive(Clone, Debug)]
pub(super) struct Changes {
    sender: broadcast::Sender<SessionChange>,
}

impl Default for Changes {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }
}

impl Changes {
    fn publish(&self, session: Session) {
        // `send` fails only when nobody is subscribed.
        let _ = self.sender.send(SessionChange { session });
    }
}

impl Live {
    #[must_use]
    pub fn changes(&self) -> broadcast::Receiver<SessionChange> {
        self.changes.sender.subscribe()
    }

    /// Unlike [`Live::publish_start`], it publishes even while the hub is shutting down.
    pub(in crate::fleet::sessions) fn publish_change(&self, changed: Session) {
        self.changes.publish(changed);
    }

    /// Publishes only while the ticket is held, so every end of the session is published after
    /// its start; a session whose ticket went first has an end with no start in the feed.
    /// Returns `false` when the hub is shutting down.
    pub(in crate::fleet::sessions) fn publish_start(&self, started: Session) -> bool {
        let state = self.state();
        if state.shutting_down {
            return false;
        }
        if state.holds_ticket(started.id) {
            // A broadcast send never waits, so it may happen under the lock.
            self.changes.publish(started);
        }
        true
    }
}
