//! The calls of the queue of a thread (TH109): only the person makes them, as the queue is the
//! composer of the person. A queued prompt is never written to the log or the record.

use agent_threads::control::{Action, Outcome, Queued, Refusal};
use uuid::Uuid;

use super::super::log::Kind;
use super::super::{Caller, Runner, Waits};
use super::send::{Sender, SentPrompt};

impl Runner {
    /// A call that changes the queue but sends nothing by itself.
    pub(super) async fn queue_call(&mut self, action: Action) -> Outcome {
        match action {
            Action::Queue { thread, text } => self.queue(thread, text).await,
            Action::Requeue {
                thread,
                queued,
                text,
            } => self.requeue(thread, queued, text).await,
            Action::Unqueue { thread, queued } => self.unqueue(thread, queued).await,
            _ => refused(Refusal::BadValue),
        }
    }

    /// A prompt that waits for the turn to end; it goes at once when the thread waits for one.
    async fn queue(&mut self, id: Uuid, text: String) -> Outcome {
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::UnknownThread);
        };
        match thread.queue(Uuid::now_v7(), text) {
            Ok(steps) => self.queue_steps(id, steps, "queued").await,
            Err(why) => refused(why),
        }
    }

    async fn requeue(&mut self, id: Uuid, queued: Uuid, text: String) -> Outcome {
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::UnknownThread);
        };
        match thread.requeue(queued, text) {
            Ok(steps) => self.queue_steps(id, steps, "requeued").await,
            Err(why) => refused(why),
        }
    }

    async fn unqueue(&mut self, id: Uuid, queued: Uuid) -> Outcome {
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::UnknownThread);
        };
        match thread.unqueue(queued) {
            Ok((_, step)) => self.queue_steps(id, vec![step], "unqueued").await,
            Err(why) => refused(why),
        }
    }

    /// A queued prompt now: it leaves the queue and goes as a prompt of the person does. When
    /// it does not go, it waits again, first in the queue, with its ID.
    pub(super) async fn send_queued(
        &mut self,
        id: Uuid,
        queued: Uuid,
        back: Caller,
    ) -> Result<(), Refusal> {
        let thread = self.threads.get_mut(&id).ok_or(Refusal::UnknownThread)?;
        let (text, step) = thread.take_queued(queued)?;
        let _ = self.apply(id, vec![step]).await;
        let send = Action::Send {
            thread: id,
            text: text.clone(),
            attachments: Vec::new(),
        };
        let went = self.send(send, back, Sender::Queue(queued)).await;
        if went.is_err() {
            self.queued_back(id, queued, text).await;
        }
        went
    }

    /// A prompt that the queue sent by itself waits for the holder to take its last line.
    pub(in super::super) fn queued_went(
        &mut self,
        thread: Uuid,
        queued: Queued,
        line: Option<u64>,
    ) {
        if let Some(line) = line {
            let prompt = SentPrompt::queued(thread, queued);
            self.waits.insert(line, Waits::Send(prompt));
        }
    }

    /// A prompt of the queue that did not go waits again, first in the queue.
    pub(in super::super) async fn queued_back(&mut self, id: Uuid, queued: Uuid, text: String) {
        let Some(step) = self
            .threads
            .get_mut(&id)
            .and_then(|thread| thread.put_back(queued, text))
        else {
            return;
        };
        let _ = self.apply(id, vec![step]).await;
    }

    async fn queue_steps(
        &mut self,
        id: Uuid,
        steps: Vec<super::Step>,
        word: &'static str,
    ) -> Outcome {
        self.note(Kind::Prompt, Some(id), word);
        if self.apply(id, steps).await.is_err() {
            return refused(Refusal::Holder);
        }
        Outcome::Ok
    }
}

const fn refused(why: Refusal) -> Outcome {
    Outcome::Refused { why }
}

#[cfg(test)]
mod tests {
    use agent_threads::control::Action;
    use uuid::Uuid;

    #[test]
    fn th109_only_the_person_changes_the_queue() {
        let (thread, queued) = (Uuid::nil(), Uuid::nil());
        let text = String::new();
        for action in [
            Action::Queue {
                thread,
                text: text.clone(),
            },
            Action::Requeue {
                thread,
                queued,
                text,
            },
            Action::Unqueue { thread, queued },
            Action::SendQueued { thread, queued },
        ] {
            assert!(super::super::for_the_person(&action), "{action:?}");
        }
    }
}
