//! The files of a draft (TH111): the person attaches them on the page of a new thread, before
//! the thread exists, under a key that the page made. They wait as the files of a thread wait,
//! with the same limits and the same hour; the start that names the key gives them to the new
//! thread for its first prompt.

use agent_threads::control::Refusal;
use uuid::Uuid;

use super::{Attachments, make_private, remove};

impl Attachments {
    /// Whether the files `ids` of the draft `draft` can go with a first prompt.
    ///
    /// # Errors
    /// [`Refusal::BadValue`] for files with no draft, and the refusals of a prompt with files.
    pub fn ready(&self, draft: Option<Uuid>, ids: &[Uuid]) -> Result<(), Refusal> {
        if ids.is_empty() {
            return Ok(());
        }
        self.check(draft.ok_or(Refusal::BadValue)?, ids)
    }

    /// Gives the files `ids` of `draft` to the new thread `thread`: they move to its folder and
    /// wait there for its first prompt. The files that moved; a file that did not stays with
    /// the draft until its hour ends.
    pub fn adopt(&mut self, draft: Option<Uuid>, thread: Uuid, ids: &[Uuid]) -> Vec<Uuid> {
        let Some(draft) = draft else {
            return Vec::new();
        };
        let moved = self.moved(draft, thread, ids);
        if !moved.is_empty() {
            self.drafts.insert(thread, draft);
        }
        moved
    }

    /// The new thread `thread` did not start: the files that it took from its draft go back to
    /// the draft, so that a new start can name them again.
    pub fn give_back(&mut self, thread: Uuid) {
        let Some(draft) = self.drafts.remove(&thread) else {
            return;
        };
        let ids: Vec<Uuid> = self
            .pending
            .iter()
            .filter(|(_, upload)| upload.thread == thread)
            .map(|(id, _)| *id)
            .collect();
        self.moved(thread, draft, &ids);
        self.forget(thread);
    }

    /// The first prompt of `thread` took its files: its draft is over.
    pub fn draft_over(&mut self, thread: Uuid) {
        self.drafts.remove(&thread);
    }

    fn moved(&mut self, draft: Uuid, thread: Uuid, ids: &[Uuid]) -> Vec<Uuid> {
        let mut moved = Vec::new();
        for id in ids {
            let mine = self
                .pending
                .get(id)
                .is_some_and(|upload| upload.thread == draft);
            let to = self.pending_folder(thread, *id);
            let went = mine
                && to.parent().is_some_and(|above| make_private(above).is_ok())
                && std::fs::rename(self.pending_folder(draft, *id), &to).is_ok();
            if let Some(upload) = self.pending.get_mut(id).filter(|_| went) {
                upload.thread = thread;
                moved.push(*id);
            }
        }
        if !self.pending.values().any(|upload| upload.thread == draft) {
            remove(&self.folder.join(draft.to_string()));
        }
        moved
    }
}
