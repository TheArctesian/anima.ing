//! The call `send`: a prompt to a thread, with the files that the person attached for it (TH111).

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::Prompt;
use uuid::Uuid;

use super::super::holder_events::taken;
use super::super::log::Kind;
use super::super::socket::Caller;
use super::super::thread::{Shown, Step, Thread};
use super::super::{Runner, Waits};
use crate::threads::wire::Outcome as HolderOutcome;

/// The most bytes of the words and the images of one line to a program: a frame to the holder
/// holds at most 32 MiB.
const MOST_LINE_BYTES: usize = 24 * 1024 * 1024;

impl Runner {
    /// A prompt. The client gets its answer when the holder took the line. `person` is false for
    /// a prompt of a process of an agent: it does not start a stopped queue (TH109), and it
    /// cannot take files, which only the person attaches.
    pub(super) async fn send(
        &mut self,
        action: Action,
        back: Caller,
        person: bool,
    ) -> Result<(), Refusal> {
        let Action::Send {
            thread: id,
            text,
            attachments,
        } = action
        else {
            return Err(Refusal::BadValue);
        };
        if !person && !attachments.is_empty() {
            return Err(Refusal::FromAnAgent);
        }
        let prompt = self.prompt_of(id, text, &attachments)?;
        let lines = match self.lines_for(id, &prompt, person) {
            Ok(lines) => lines,
            Err(why) => {
                self.attached.restore(id, &attachments);
                return Err(why);
            }
        };
        let shown = Shown::of(&prompt);
        let steps = lines.into_iter().map(Step::Write).collect();
        match self.apply(id, steps).await {
            Err(_) => {
                self.attached.restore(id, &attachments);
                Err(Refusal::Holder)
            }
            Ok(Some(command)) => {
                let waits = Waits::Send {
                    thread: id,
                    back,
                    shown,
                    files: attachments,
                };
                self.waits.insert(command, waits);
                Ok(())
            }
            // A Codex prompt that waits in the thread for its ID: it is the thread's now.
            Ok(None) => {
                self.attached.settle(&attachments);
                self.note(Kind::Prompt, Some(id), "sent");
                let _ = self.apply(id, Thread::sent(&shown)).await;
                back.answer(Outcome::Ok);
                Ok(())
            }
        }
    }

    /// A prompt with its files, taken from where they wait. A refusal leaves the files waiting
    /// for a later prompt.
    fn prompt_of(&mut self, id: Uuid, text: String, files: &[Uuid]) -> Result<Prompt, Refusal> {
        let thread = self.threads.get(&id).ok_or(Refusal::UnknownThread)?;
        thread.can_take_a_prompt()?;
        let program = thread.program();
        let attachments = self.attached.take(id, files, program)?;
        let prompt = Prompt { text, attachments };
        // The text is written twice escaped at worst: in the line, and in the frame to the holder.
        let inline: usize = prompt
            .attachments
            .iter()
            .filter_map(|file| file.kind.inline())
            .map(str::len)
            .sum();
        if prompt.text.len() * 2 + inline > MOST_LINE_BYTES {
            self.attached.restore(id, files);
            return Err(Refusal::TooLarge {
                said: "The prompt with its images is too large for one line: send fewer images with it.".to_owned(),
            });
        }
        Ok(prompt)
    }

    /// The lines of a prompt to its thread.
    fn lines_for(
        &mut self,
        id: Uuid,
        prompt: &Prompt,
        person: bool,
    ) -> Result<Vec<String>, Refusal> {
        self.threads
            .get_mut(&id)
            .ok_or(Refusal::UnknownThread)
            .and_then(|thread| thread.send(prompt, person))
    }

    /// The holder answered a prompt: it is shown as sent, or refused.
    pub(in super::super) async fn sent(
        &mut self,
        id: Uuid,
        shown: &Shown,
        outcome: &HolderOutcome,
    ) -> Outcome {
        if let Some(why) = taken(outcome) {
            return Outcome::Refused { why };
        }
        self.note(Kind::Prompt, Some(id), "sent");
        let _ = self.apply(id, Thread::sent(shown)).await;
        Outcome::Ok
    }

    /// The files of a prompt stay where the program reads them when it went, and wait again
    /// when it was refused.
    pub(in super::super) fn files_went(&mut self, thread: Uuid, files: &[Uuid], result: &Outcome) {
        if matches!(result, Outcome::Refused { .. }) {
            self.attached.restore(thread, files);
        } else {
            self.attached.settle(files);
        }
    }
}
