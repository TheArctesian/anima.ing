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
        let relayed = thread.row.provider_address.is_some();
        if relayed {
            self.note(Kind::Relay, Some(id), "pass-through started");
            self.publish(id, agent_threads::control::Change::Said {
                event: agent_threads::drive::DriveEvent::Notice {
                    text: "This thread uses the local model relay in pass-through. Nothing is shortened.".to_owned(),
                },
            });
        }
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
            // The launcher lets go of the seat of the thread at once.
            self.rights_now();
            match outcome {
                HolderOutcome::Refused { why: crate::threads::wire::Refusal::RelayFailed } => refused(Refusal::ProviderAddress {
                    said: "The local model relay could not start. No agent was started.".to_owned(),
                }),
                HolderOutcome::Refused { why: crate::threads::wire::Refusal::ProviderConflict } => refused(Refusal::ProviderAddress {
                    said: "Claude Code settings set ANTHROPIC_BASE_URL or could not be checked. Remove the conflicting setting before using the relay.".to_owned(),
                }),
                _ => refused(Refusal::Holder),
            }
        } else {
            self.began(id, prompt, files).await
        }
    }
}
