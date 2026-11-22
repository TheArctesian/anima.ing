//! The prompts that wait for the turn of a thread to end (TH109, THR-102): they go one by one,
//! each as the next turn when the turn before it ended well with nothing open for an answer.
//! The queue stops, and keeps its prompts, after an interrupt and after a turn that failed,
//! until the person sends. It is in memory only. Pure.

use agent_threads::control::{
    Change, LastTurn, MOST_QUEUED, MOST_QUEUED_BYTES, Queued, Refusal, RunState,
};
use agent_threads::drive::{ProgramState, Prompt, TurnOutcome};
use uuid::Uuid;

use super::{Step, Thread, prompt_change};

impl Thread {
    /// Puts a prompt at the end of the queue; it goes at once when the thread waits for a
    /// prompt and the queue did not stop.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended or did not start yet, and
    /// [`Refusal::QueueFull`] when [`MOST_QUEUED`] prompts wait, and [`Refusal::BadValue`] for
    /// an empty prompt or one over [`MOST_QUEUED_BYTES`].
    pub fn queue(&mut self, id: Uuid, text: String) -> Result<Vec<Step>, Refusal> {
        self.running()?;
        fits(&text)?;
        if self.row.queued.len() >= MOST_QUEUED {
            return Err(Refusal::QueueFull);
        }
        self.row.queued.push(Queued { id, text });
        let mut steps = vec![self.row_change()];
        steps.extend(self.next_queued());
        Ok(steps)
    }

    /// New words for a prompt that waits.
    ///
    /// # Errors
    /// [`Refusal::RequestGone`] when the prompt went or was taken out, and [`Refusal::BadValue`]
    /// for words that do not fit.
    pub fn requeue(&mut self, id: Uuid, text: String) -> Result<Vec<Step>, Refusal> {
        fits(&text)?;
        let waiting = self.row.queued.iter_mut().find(|queued| queued.id == id);
        waiting.ok_or(Refusal::RequestGone)?.text = text;
        Ok(vec![self.row_change()])
    }

    /// Takes a prompt out of the queue, to send it now or never.
    ///
    /// # Errors
    /// [`Refusal::RequestGone`] when the prompt went or was taken out.
    pub fn unqueue(&mut self, id: Uuid) -> Result<(String, Step), Refusal> {
        let at = self.row.queued.iter().position(|queued| queued.id == id);
        let queued = self.row.queued.remove(at.ok_or(Refusal::RequestGone)?);
        Ok((queued.text, self.row_change()))
    }

    /// Takes a prompt out of the queue to send it now, while the program runs.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended, and [`Refusal::RequestGone`] when the
    /// prompt went or was taken out.
    pub fn take_queued(&mut self, id: Uuid) -> Result<(String, Step), Refusal> {
        self.running()?;
        self.unqueue(id)
    }

    /// A turn ended: an interrupt or a failure stops the queue.
    pub(super) fn turn_ended(&mut self, outcome: TurnOutcome) {
        let last = match outcome {
            TurnOutcome::Completed => LastTurn::Done,
            TurnOutcome::Failed => LastTurn::Failed,
            TurnOutcome::Interrupted => LastTurn::Interrupted,
        };
        self.pending = self.pending.saturating_sub(1);
        // Only the end of the last turn that went stops the queue: a late end of a turn that the
        // person stopped before a new prompt does not stop it again.
        self.stalled |= last != LastTurn::Done && self.pending == 0;
        self.row.last_turn = Some(last);
    }

    /// The person stopped the turn: nothing more goes by itself.
    pub(super) const fn stall(&mut self) {
        self.stalled = true;
    }

    /// A prompt of the queue that did not go waits again, first, with its ID. Nothing waits on
    /// a stopped thread: a stop ends the queue (THR-102).
    pub fn put_back(&mut self, id: Uuid, text: String) -> Option<Step> {
        if self.row.state == RunState::Stopped
            || self.row.queued.iter().any(|queued| queued.id == id)
        {
            return None;
        }
        self.row.queued.insert(0, Queued { id, text });
        Some(self.row_change())
    }

    /// The first prompt of the queue as the next turn, when the program waits for one with
    /// nothing open for an answer, and the queue did not stop.
    pub(super) fn next_queued(&mut self) -> Vec<Step> {
        let waits = self.held
            && self.row.state != RunState::Stopped
            && self.open.is_empty()
            && self.program == Some(ProgramState::Idle);
        if !waits || self.pending > 0 || self.stalled || self.row.queued.is_empty() {
            return Vec::new();
        }
        let Queued { id, text } = self.row.queued.remove(0);
        let Ok(lines) = self.send(&Prompt::plain(&text), false) else {
            self.row.queued.insert(0, Queued { id, text });
            return Vec::new();
        };
        let mut steps: Vec<Step> = lines.into_iter().map(Step::Write).collect();
        steps.push(prompt_change(&text));
        steps.push(self.row_change());
        steps
    }

    /// Whether the thread can take a prompt now.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended or the holder did not start it yet.
    pub fn can_take_a_prompt(&self) -> Result<(), Refusal> {
        self.running()
    }

    fn running(&self) -> Result<(), Refusal> {
        if self.row.state == RunState::Stopped || !self.held {
            return Err(Refusal::NotRunning);
        }
        Ok(())
    }

    /// The row as it is now, for the clients.
    pub(super) fn row_change(&self) -> Step {
        Step::Change(Change::State {
            state: self.row.state,
            row: self.row.clone(),
        })
    }
}

fn fits(text: &str) -> Result<(), Refusal> {
    if text.trim().is_empty() || text.len() > MOST_QUEUED_BYTES {
        return Err(Refusal::BadValue);
    }
    Ok(())
}
