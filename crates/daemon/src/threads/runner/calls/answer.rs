//! An answer of the person to a request of a program (THR-030).

use agent_threads::control::Refusal;
use agent_threads::drive::Answer;
use uuid::Uuid;

use super::super::socket::Caller;
use super::super::thread::Step;
use super::super::{Pending, Runner, Waits};

/// The request of a thread that an answer is for.
pub(in crate::threads::runner) struct Answering {
    pub thread: Uuid,
    pub request: Uuid,
}

impl Runner {
    /// An answer of the person. The request closes, and the client gets its answer, when the
    /// holder took the line (THR-030).
    pub(in crate::threads::runner) async fn answer(
        &mut self,
        to: Answering,
        answer: &Answer,
        back: Caller,
    ) -> Result<(), Refusal> {
        let Answering {
            thread: id,
            request,
        } = to;
        let thread = self.threads.get_mut(&id).ok_or(Refusal::UnknownThread)?;
        let line = thread.answer(request, answer)?;
        // The record says that the answer went, before the line goes (THR-030).
        self.save();
        let Ok(Some(command)) = self.apply(id, vec![Step::Write(line)]).await else {
            if let Some(thread) = self.threads.get_mut(&id) {
                thread.answer_failed(request);
            }
            // The record must not say that the answer went: a restart would close the request.
            self.save();
            return Err(Refusal::Holder);
        };
        let pending = Pending {
            thread: id,
            request,
            word: answer_word(answer),
            back,
        };
        self.waits
            .insert(command, self.link.origin(), Waits::Answer(pending));
        Ok(())
    }
}

const fn answer_word(answer: &Answer) -> &'static str {
    match answer {
        Answer::Allow => "allow",
        Answer::AllowForSession { .. } => "allow for the session",
        Answer::Deny { .. } => "deny",
        Answer::Choose { .. } => "choose",
    }
}
