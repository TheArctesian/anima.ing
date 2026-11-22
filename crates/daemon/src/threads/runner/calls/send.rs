//! The call `send`: a prompt to a thread, with the files that the person attached for it (TH111).

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::Prompt;

use super::super::log::Kind;
use super::super::socket::Caller;
use super::super::thread::{Shown, Step, Thread};
use super::super::{Runner, Waits};

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
        let thread = self.threads.get(&id).ok_or(Refusal::UnknownThread)?;
        // A thread that cannot take the prompt keeps its files for a later prompt.
        thread.can_take_a_prompt()?;
        let program = thread.program();
        let attachments = self.attached.take(id, &attachments, program)?;
        let prompt = Prompt { text, attachments };
        let thread = self.threads.get_mut(&id).ok_or(Refusal::UnknownThread)?;
        let lines = thread.send(&prompt, person)?;
        let shown = Shown::of(&prompt);
        drop(prompt);
        let steps = lines.into_iter().map(Step::Write).collect();
        match self.apply(id, steps).await {
            Err(_) => Err(Refusal::Holder),
            Ok(Some(command)) => {
                let waits = Waits::Send {
                    thread: id,
                    back,
                    shown,
                };
                self.waits.insert(command, waits);
                Ok(())
            }
            // A Codex prompt that waits in the thread for its ID: it is the thread's now.
            Ok(None) => {
                self.note(Kind::Prompt, Some(id), "sent");
                let _ = self.apply(id, Thread::sent(&shown)).await;
                back.answer(Outcome::Ok);
                Ok(())
            }
        }
    }
}
