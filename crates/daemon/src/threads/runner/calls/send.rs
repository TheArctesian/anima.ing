//! The call `send`: a prompt to a thread, with the files that the person attached for it (TH111)
//! and the skills that the person chose for it (TH117).

use agent_threads::control::{Action, Outcome, Queued, Refusal};
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

/// Who sends a prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Sender {
    /// The person, from a client.
    Person,
    /// A process of an agent: its prompt does not start a stopped queue (TH109), and it cannot
    /// take files, which only the person attaches.
    Agent,
    /// The person, with a prompt of the queue: it goes back to the queue when it does not go.
    Queue(Uuid),
}

/// A prompt whose last line waits for the holder.
#[derive(Debug)]
pub struct SentPrompt {
    thread: Uuid,
    /// The call that waits for the answer; none for a prompt that the queue sent by itself.
    pub back: Option<Caller>,
    shown: Shown,
    /// The files of the prompt, which wait again when the holder does not take the line.
    files: Vec<Uuid>,
    /// The prompt of the queue that this is, which waits again when the holder does not take
    /// the line.
    queued: Option<Uuid>,
    /// The prompt counted as a turn of its own: it does not when the holder refuses it.
    counted: bool,
}

impl SentPrompt {
    /// A prompt that the queue sent by itself, as its next turn.
    pub(in super::super) fn queued(thread: Uuid, queued: Queued) -> Self {
        Self {
            thread,
            back: None,
            shown: Shown {
                text: queued.text,
                ..Shown::default()
            },
            files: Vec::new(),
            queued: Some(queued.id),
            counted: true,
        }
    }
}

impl Runner {
    /// A prompt. The client gets its answer when the holder took the line.
    pub(super) async fn send(
        &mut self,
        action: Action,
        back: Caller,
        sender: Sender,
    ) -> Result<(), Refusal> {
        let person = sender != Sender::Agent;
        let Action::Send {
            thread: id,
            text,
            attachments,
            skills,
        } = action
        else {
            return Err(Refusal::BadValue);
        };
        let has_more = !attachments.is_empty() || !skills.is_empty();
        if !person && has_more {
            return Err(Refusal::FromAnAgent);
        }
        self.threads
            .get(&id)
            .ok_or(Refusal::UnknownThread)?
            .takes(&skills)?;
        let mut prompt = self.prompt_of(id, text, &attachments)?;
        prompt.skills = skills;
        let (lines, counted) = match self.lines_for(id, &prompt, person) {
            Ok(went) => went,
            Err(why) => {
                self.attached.restore(id, &attachments);
                return Err(why);
            }
        };
        self.persist_send(id, counted, &attachments)?;
        let shown = Shown::of(&prompt);
        let steps = lines.into_iter().map(Step::Write).collect();
        match self.apply(id, steps).await {
            Err(_) => {
                self.attached.restore(id, &attachments);
                Err(Refusal::Holder)
            }
            Ok(Some(command)) => {
                let waits = Waits::Send(SentPrompt {
                    thread: id,
                    back: Some(back),
                    shown,
                    files: attachments,
                    queued: match sender {
                        Sender::Queue(queued) => Some(queued),
                        Sender::Person | Sender::Agent => None,
                    },
                    counted,
                });
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

    fn persist_send(
        &mut self,
        id: Uuid,
        counted: bool,
        attachments: &[Uuid],
    ) -> Result<(), Refusal> {
        let grok = self
            .threads
            .get(&id)
            .is_some_and(|thread| thread.program() == agent_threads::drive::Program::Grok);
        // Persist the reply ID before dispatch. After a crash an uncertain send is never replayed.
        if grok && !self.written() {
            if let Some(thread) = self.threads.get_mut(&id) {
                thread.not_sent(counted);
            }
            self.attached.restore(id, attachments);
            return Err(Refusal::Holder);
        }
        Ok(())
    }

    /// A prompt with its files, taken from where they wait. A refusal leaves the files waiting
    /// for a later prompt.
    fn prompt_of(&mut self, id: Uuid, text: String, files: &[Uuid]) -> Result<Prompt, Refusal> {
        let thread = self.threads.get(&id).ok_or(Refusal::UnknownThread)?;
        thread.can_take_a_prompt()?;
        let program = thread.program();
        let attachments = self.attached.take(id, files, program)?;
        let prompt = Prompt {
            text,
            attachments,
            skills: Vec::new(),
        };
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

    /// The lines of a prompt to its thread, and whether it counted as a turn of its own.
    fn lines_for(
        &mut self,
        id: Uuid,
        prompt: &Prompt,
        person: bool,
    ) -> Result<(Vec<String>, bool), Refusal> {
        let thread = self.threads.get_mut(&id).ok_or(Refusal::UnknownThread)?;
        let before = thread.turns_waiting();
        let lines = thread.send(prompt, person)?;
        Ok((lines, thread.turns_waiting() > before))
    }

    /// The holder answered a prompt: it is shown as sent, or refused, and its files and its
    /// place in the queue follow; gives the thread and the answer for the client.
    pub(in super::super) async fn prompt_answered(
        &mut self,
        prompt: &SentPrompt,
        outcome: &HolderOutcome,
    ) -> (Uuid, Outcome) {
        let thread = prompt.thread;
        let result = self.sent(thread, &prompt.shown, outcome).await;
        self.files_went(thread, &prompt.files, &result);
        if matches!(result, Outcome::Refused { .. }) {
            // A prompt that did not go leaves no turn to wait for: the queue may go on, but not
            // with the refused prompt of the queue itself, which waits for the person.
            let next = self.threads.get_mut(&thread).map(|waiting| {
                waiting.not_sent(prompt.counted);
                if prompt.queued.is_none() {
                    waiting.next_queued()
                } else {
                    Vec::new()
                }
            });
            let _ = self.apply(thread, next.unwrap_or_default()).await;
            if let Some(queued) = prompt.queued {
                self.queued_back(thread, queued, prompt.shown.text.clone())
                    .await;
            }
        }
        (thread, result)
    }

    /// The holder answered a prompt: it is shown as sent, or refused.
    async fn sent(&mut self, id: Uuid, shown: &Shown, outcome: &HolderOutcome) -> Outcome {
        if let Some(why) = taken(outcome) {
            return Outcome::Refused { why };
        }
        self.note(Kind::Prompt, Some(id), "sent");
        let steps = self
            .threads
            .get_mut(&id)
            .map_or_else(Vec::new, |thread| thread.sent_to(shown));
        let _ = self.apply(id, steps).await;
        self.save();
        Outcome::Ok
    }

    /// The files of a prompt stay where the program reads them when it went, and wait again
    /// when it was refused.
    fn files_went(&mut self, thread: Uuid, files: &[Uuid], result: &Outcome) {
        if matches!(result, Outcome::Refused { .. }) {
            self.attached.restore(thread, files);
        } else {
            self.attached.settle(files);
        }
    }
}
