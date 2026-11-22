//! The record of a thread: what the runner writes, so that a runner that starts again has the
//! thread, its open requests and what the person changed (THR-030, TH110).

use agent_threads::control::{RunState, ThreadRow};
use agent_threads::drive::{DriveEvent, ProgramState};
use uuid::Uuid;

use super::super::codex::{Changed, Codex};
use super::{Driver, Open, Saved, Thread};

impl Thread {
    /// The record of the thread.
    #[must_use]
    pub fn saved(&self) -> Saved {
        let (codex_thread, codex_turn, codex_changed) = match &self.driver {
            Driver::Codex(codex) => (codex.thread.clone(), codex.turn.clone(), codex.changed()),
            Driver::Claude(_) => (None, None, Changed::default()),
        };
        Saved {
            row: self.recorded_row(),
            effort: self.row.effort,
            codex_thread,
            codex_turn,
            codex_changed,
            open: self
                .open
                .iter()
                .map(|(id, open)| (*id, open.line.clone()))
                .collect(),
            seq: self.seq,
            asked: self.asked.clone(),
            answering: self
                .open
                .iter()
                .filter(|(_, open)| open.answering)
                .map(|(id, _)| *id)
                .collect(),
        }
    }

    /// A turn of Codex that leaves the profile of the folder of Anima waits (THR-041): the
    /// record must say so before its line goes.
    #[must_use]
    pub fn leaves_profile(&self) -> bool {
        matches!(&self.driver, Driver::Codex(codex) if codex.leaving())
    }

    /// What the record holds of this thread but the lines of its requests: when it does not
    /// change, the record need not be written again.
    #[must_use]
    pub fn record_key(&self) -> String {
        let (codex_thread, codex_turn, codex_changed) = match &self.driver {
            Driver::Codex(codex) => (
                codex.thread.as_deref(),
                codex.turn.as_deref(),
                Some(codex.changed()),
            ),
            Driver::Claude(_) => (None, None, None),
        };
        let open: Vec<(&Uuid, bool)> = self
            .open
            .iter()
            .map(|(id, open)| (id, open.answering))
            .collect();
        serde_json::to_string(&(
            &self.recorded_row(),
            codex_thread,
            codex_turn,
            codex_changed,
            open,
        ))
        .unwrap_or_default()
    }

    /// The row as the record holds it: never the prompts that wait (TH109).
    fn recorded_row(&self) -> ThreadRow {
        ThreadRow {
            queued: Vec::new(),
            ..self.row.clone()
        }
    }

    /// A thread from its record, after a start of the runner; its open requests are read again
    /// from their lines and keep their IDs.
    #[must_use]
    pub fn restore(saved: Saved, codex: Option<Codex>) -> Self {
        let codex = codex
            .map(|codex| codex.known(saved.codex_thread, saved.codex_turn, saved.codex_changed));
        let mut thread = Self::new(saved.row, saved.effort, codex);
        // What the program last said stays true until it says more (THR-085).
        thread.program = match thread.row.state {
            RunState::Idle => Some(ProgramState::Idle),
            RunState::Working | RunState::NeedsYou => Some(ProgramState::Working),
            RunState::Starting | RunState::Stopped | RunState::Unknown => None,
        };
        thread.held = thread.row.state != RunState::Stopped;
        thread.seq = saved.seq;
        thread.asked = saved.asked;
        for (id, line) in saved.open {
            let events = thread.decode(line.as_bytes());
            if let Some(DriveEvent::Request(request)) = events.into_iter().next() {
                thread.open.insert(
                    id,
                    Open {
                        request,
                        line,
                        answering: saved.answering.contains(&id),
                    },
                );
            }
        }
        thread
    }
}
