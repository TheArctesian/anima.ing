//! The prompts that wait for the turn of a thread to end (TH109, THR-102): they go one by one,
//! each as the next turn when the turn before it ended well with nothing open for an answer.
//! The queue stops, and keeps its prompts, after an interrupt and after a turn that failed,
//! until the person sends. It is in memory only. Pure.

use agent_threads::control::{Change, LastTurn, MOST_QUEUED, Queued, Refusal, RunState};
use agent_threads::drive::{ProgramState, TurnOutcome};
use uuid::Uuid;

use super::{Step, Thread, prompt_change};

impl Thread {
    /// Puts a prompt at the end of the queue; it goes at once when the thread waits for a
    /// prompt and the queue did not stop.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended or did not start yet, and
    /// [`Refusal::QueueFull`] when [`MOST_QUEUED`] prompts wait.
    pub fn queue(&mut self, id: Uuid, text: String) -> Result<Vec<Step>, Refusal> {
        self.running()?;
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
    /// [`Refusal::RequestGone`] when the prompt went or was taken out.
    pub fn requeue(&mut self, id: Uuid, text: String) -> Result<Vec<Step>, Refusal> {
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
        self.awaiting = false;
        self.stalled |= last != LastTurn::Done;
        self.row.last_turn = Some(last);
    }

    /// The person stopped the turn: nothing more goes by itself.
    pub(super) const fn stall(&mut self) {
        self.stalled = true;
    }

    /// The first prompt of the queue as the next turn, when the program waits for one with
    /// nothing open for an answer, and the queue did not stop.
    pub(super) fn next_queued(&mut self) -> Vec<Step> {
        let waits = self.held
            && self.row.state != RunState::Stopped
            && self.open.is_empty()
            && self.program == Some(ProgramState::Idle);
        if !waits || self.awaiting || self.stalled || self.row.queued.is_empty() {
            return Vec::new();
        }
        let Queued { text, .. } = self.row.queued.remove(0);
        let mut steps: Vec<Step> = match self.send(&text) {
            Ok(lines) => lines.into_iter().map(Step::Write).collect(),
            Err(_) => return Vec::new(),
        };
        steps.push(prompt_change(&text));
        steps.push(self.row_change());
        steps
    }

    fn running(&self) -> Result<(), Refusal> {
        if self.row.state == RunState::Stopped || !self.held {
            return Err(Refusal::NotRunning);
        }
        Ok(())
    }

    /// The row as it is now, for the clients.
    fn row_change(&self) -> Step {
        Step::Change(Change::State {
            state: self.row.state,
            row: self.row.clone(),
        })
    }
}
