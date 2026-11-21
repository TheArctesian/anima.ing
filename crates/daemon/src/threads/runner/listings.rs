//! How the person lists each thread (TH46): pinned, archived, a title of their own, or removed.
//! Kept in the record of the runner only; no session file of a program changes.

use agent_threads::control::{Listing, ListingChange, Outcome, Refusal, RunState};
use uuid::Uuid;

use super::Runner;

/// The most listings the record keeps: a list of threads, not a store of text.
const MAX_LISTINGS: usize = 10_000;

impl Runner {
    /// Each listing that says more than the files.
    pub(super) fn listings(&self) -> Outcome {
        Outcome::Listings {
            listings: self.listings.values().cloned().collect(),
        }
    }

    /// Changes the listing of `thread`, and answers with each listing.
    pub(super) fn change_listing(&mut self, thread: Uuid, change: &ListingChange) -> Outcome {
        match self.listing_changed(thread, change) {
            Ok(()) => {
                self.save();
                self.listings()
            }
            Err(why) => Outcome::Refused { why },
        }
    }

    fn listing_changed(&mut self, thread: Uuid, change: &ListingChange) -> Result<(), Refusal> {
        let listing = self
            .listings
            .get(&thread)
            .cloned()
            .unwrap_or_else(|| Listing {
                thread,
                ..Listing::default()
            })
            .changed(change)
            .map_err(|_| Refusal::BadValue)?;
        if !self.listings.contains_key(&thread) && self.listings.len() >= MAX_LISTINGS {
            return Err(Refusal::BadValue);
        }
        if *change == ListingChange::Remove {
            self.forget(thread)?;
        }
        if listing.is_plain() {
            self.listings.remove(&thread);
        } else {
            self.listings.insert(thread, listing);
        }
        Ok(())
    }

    /// Forgets a thread that the runner started, after it stopped; its session, which is also
    /// a file of its program, is removed from the list with it.
    fn forget(&mut self, thread: Uuid) -> Result<(), Refusal> {
        // A thread is named by its own ID or by the session of its program (Codex has its own).
        let named = thread.to_string();
        let runs_as_session = self.threads.values().any(|started| {
            started.row.session.as_deref() == Some(named.as_str())
                && started.row.state != RunState::Stopped
        });
        if runs_as_session {
            return Err(Refusal::Running);
        }
        let Some(started) = self.threads.get(&thread) else {
            return Ok(());
        };
        if started.row.state != RunState::Stopped {
            return Err(Refusal::Running);
        }
        let session = started
            .row
            .session
            .as_deref()
            .and_then(|id| id.parse().ok());
        self.threads.remove(&thread);
        self.read_up_to.remove(&thread);
        if let Some(session) = session.filter(|session| *session != thread) {
            let removed = Listing {
                thread: session,
                removed: true,
                ..Listing::default()
            };
            self.listings.insert(session, removed);
        }
        Ok(())
    }
}
