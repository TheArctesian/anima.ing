//! The firings that go to the open applications over `Subscription.reminderFirings` (K-E6
//! Section 5; REM-008, REM-009).

use tokio::sync::broadcast;

use super::{
    delivery::lock_screen,
    model::{Class, Firing},
};

/// The most firings that wait for one slow subscriber; it then skips the oldest.
const CAPACITY: usize = 64;

/// The hub's one way to the open applications. `main.rs` builds it once: the schema and the
/// sender hold clones of this same value, or the sender counts no application.
#[derive(Clone, Debug)]
pub struct Live {
    sender: broadcast::Sender<Firing>,
}

impl Default for Live {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }
}

impl Live {
    /// A subscription that the client closed is not counted after its stream is dropped.
    #[must_use]
    pub fn open(&self) -> usize {
        self.sender.receiver_count()
    }

    /// One open subscription until it is dropped.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Firing> {
        self.sender.subscribe()
    }

    pub(super) fn publish(&self, firing: Firing) {
        // Fails only with no subscription open; the firing is in the database, so nothing is lost.
        let _ = self.sender.send(for_a_device(firing));
    }
}

/// Class A or B leaves with the fixed body and no subject, so no title or value of a private record
/// is in the payload (REM-009).
#[must_use]
pub fn for_a_device(firing: Firing) -> Firing {
    match firing.class {
        Class::C => firing,
        Class::A | Class::B => Firing {
            subject: String::new(),
            text: lock_screen(firing.class, "", "").1,
            ..firing
        },
    }
}
