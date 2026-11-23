//! The answer of the holder to the start of a program: the first prompt goes to it, with the
//! files of its draft (TH111).

use agent_threads::control::{Outcome, Refusal, RunState};
use uuid::Uuid;

use super::super::Runner;
use super::super::log::Kind;
use super::refused;
use crate::threads::wire::Outcome as HolderOutcome;

impl Runner {
    /// The program of a new thread runs: its first prompt goes to it.
    pub(super) async fn began(&mut self, id: Uuid, prompt: &str, files: &[Uuid]) -> Outcome {
        if self
            .threads
            .get(&id)
            .is_none_or(|thread| thread.row.state == RunState::Stopped)
        {
            self.attached.give_back(id);
            return refused(Refusal::NotRunning);
        }
        let prompt = self.first_prompt(id, prompt, files);
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::Holder);
        };
        let steps = thread.begin_with(&prompt);
        let folder = thread.row.folder.clone();
        self.note(Kind::Start, Some(id), "started");
        let went = self.apply(id, steps).await.is_ok();
        let taken = if prompt.attachments.is_empty() {
            &[]
        } else {
            files
        };
        self.first_files_went(id, taken, went);
        self.save();
        Outcome::Started { thread: id, folder }
    }

    pub(super) async fn start_done(
        &mut self,
        id: Uuid,
        first: (&str, &[Uuid]),
        outcome: &HolderOutcome,
    ) -> Outcome {
        let (prompt, files) = first;
        if matches!(outcome, HolderOutcome::Refused { .. }) {
            self.threads.remove(&id);
            self.attached.give_back(id);
            refused(Refusal::Holder)
        } else {
            self.began(id, prompt, files).await
        }
    }
}
