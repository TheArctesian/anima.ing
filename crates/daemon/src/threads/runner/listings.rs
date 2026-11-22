//! How the person lists each thread (TH46): pinned, archived, a title of their own, or removed;
//! settled or snoozed (TH61); and the projects that group the threads (TH107). Kept in the record
//! of the runner only; no session file of a program changes.

use agent_threads::control::{
    Listing, ListingChange, Outcome, ProjectChange, Refusal, RunState, project_changed,
};
use uuid::Uuid;

use super::Runner;

/// The most listings the record keeps: a list of threads, not a store of text.
const MAX_LISTINGS: usize = 10_000;

impl Runner {
    pub(super) fn projects(&self) -> Outcome {
        Outcome::Projects {
            projects: self.projects.clone(),
        }
    }

    /// Changes the projects, and answers with each project. An added folder must be a folder of
    /// this computer (THR-041); a folder that is gone can still be renamed or removed.
    pub(super) fn change_project(&mut self, change: &ProjectChange) -> Outcome {
        let folder = match change {
            ProjectChange::Add { folder, .. } => {
                match super::start::chosen_folder(folder, self.dir.parent().unwrap_or(&self.dir)) {
                    Ok(real) => real,
                    Err(why) => return Outcome::Refused { why },
                }
            }
            // A folder that is gone is named as it was added.
            other => {
                std::fs::canonicalize(other.folder()).unwrap_or_else(|_| other.folder().clone())
            }
        };
        match project_changed(self.projects.clone(), change, folder) {
            Ok(projects) => {
                self.projects = projects;
                self.save();
                self.projects()
            }
            Err(_) => Outcome::Refused {
                why: Refusal::BadValue,
            },
        }
    }

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
            .changed(change, chrono::Utc::now())
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
        self.attached.forget(thread);
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
