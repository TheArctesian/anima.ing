//! What the program of a thread says: each line of its standard output read as events, and
//! what each event changes in the thread. Pure.

use agent_threads::control::RunState;
use agent_threads::drive::{DriveEvent, ProgramRequestId, ProgramState};
use uuid::Uuid;

use super::{Driver, Step, Thread, closed, said};

impl Thread {
    /// One line of standard output of the program.
    pub fn line(&mut self, text: &str, new_id: &mut impl FnMut() -> Uuid) -> Vec<Step> {
        let events = self.decode(text.as_bytes());
        let ended = events
            .iter()
            .any(|event| matches!(event, DriveEvent::TurnEnded { .. }));
        let used = self.row.usage.clone();
        let mut steps = Vec::new();
        for event in events {
            steps.extend(self.event(event, text, new_id));
        }
        // A new use of tokens goes to the clients with the row when no turn runs: the change of
        // the state at the end of a turn carries the use of that turn, and Codex says it many
        // times within a turn (TH112).
        let state = self.state_change();
        let fresh = state.is_none() && self.row.usage != used && self.row.state == RunState::Idle;
        steps.extend(state.or_else(|| fresh.then(|| self.row_change())));
        // A change of Codex lands when the turn ended, before the next queued prompt goes.
        steps.extend(self.codex_landed());
        steps.extend(self.next_queued());
        // Last, so that the row with the new branch is the last row that the clients get.
        if ended {
            steps.push(Step::Workspace);
        }
        steps
    }

    fn event(
        &mut self,
        event: DriveEvent,
        line: &str,
        new_id: &mut impl FnMut() -> Uuid,
    ) -> Vec<Step> {
        match event {
            DriveEvent::Request(request) => self.asked(request, line, new_id()),
            DriveEvent::RequestClosed { id } => self.program_closed(&id),
            DriveEvent::Reply { id, ok, result } => self.reply(&id, ok, &result),
            DriveEvent::Account { facts } => vec![Step::Account(facts)],
            DriveEvent::Models { models } => vec![Step::Models(models)],
            // A line that could wait for an answer stops the turn (THR-046: fail closed). The
            // open requests stay: only an answer, a stop by the person or the end of the process
            // closes one (THR-087).
            DriveEvent::Unknown {
                stops_turn: true, ..
            } => {
                let mut steps = vec![said(event)];
                steps.extend(self.turn_stop());
                steps.push(Step::Log("turn stopped: a line the runner does not know"));
                steps
            }
            other => {
                // The processes below the program now (its MCP servers) are spared by a stop of
                // a turn: Codex marks when a turn starts; Claude Code says its session (with its
                // servers up) before it works on a prompt.
                let mark = match self.driver {
                    Driver::Codex(_) => matches!(other, DriveEvent::TurnStarted { .. }),
                    Driver::Claude(_) => matches!(other, DriveEvent::Session { .. }),
                };
                self.track(&other);
                let mut steps: Vec<Step> = mark.then_some(Step::Mark).into_iter().collect();
                steps.push(said(other));
                steps
            }
        }
    }

    /// What an event says of the session, the turn and the state of the program.
    fn track(&mut self, event: &DriveEvent) {
        match event {
            DriveEvent::Session { id, model, .. } => {
                self.row.session = Some(id.clone());
                if model.is_some() {
                    self.row.model.clone_from(model);
                }
            }
            DriveEvent::TurnStarted { turn } => {
                self.program = Some(ProgramState::Working);
                if let Driver::Codex(codex) = &mut self.driver {
                    codex.turn_started(turn.clone());
                }
            }
            DriveEvent::State { state } => self.program = Some(*state),
            DriveEvent::Usage { usage } => self.row.usage = Some(usage.clone()),
            DriveEvent::TurnEnded { outcome, .. } => {
                self.program = Some(ProgramState::Idle);
                self.turn_ended(*outcome);
                if let Driver::Codex(codex) = &mut self.driver {
                    codex.turn_ended();
                }
            }
            _ => {}
        }
    }

    /// The line of a turn that leaves the profile of the folder of Anima did not go, because the
    /// record could not say so first (THR-041): the turn ends as one that Codex refused, so
    /// nothing waits for it and the profile stays.
    pub fn held_back(&mut self, line: &str) -> Vec<Step> {
        let Driver::Codex(codex) = &mut self.driver else {
            return Vec::new();
        };
        let Some(number) = codex.leaving_line(line) else {
            return Vec::new();
        };
        // Codex never saw the line: its call ends as a refused one, with no lines to write.
        let _ = codex.reply(number, false, &serde_json::json!({}));
        let _ = codex.refused_turn();
        self.turn_ended(agent_threads::drive::TurnOutcome::Failed);
        vec![said(DriveEvent::Notice {
            text: "Anima could not write its record, so this turn, which leaves the sandbox of \
                   the folder, did not go. Make room on the disk, then send again."
                .to_owned(),
        })]
    }

    fn reply(&mut self, id: &ProgramRequestId, ok: bool, result: &serde_json::Value) -> Vec<Step> {
        let (Driver::Codex(codex), ProgramRequestId::Number(number)) = (&mut self.driver, id)
        else {
            return self.change_answered(id, ok);
        };
        match codex.reply(*number, ok, result) {
            Ok(lines) => {
                let refused = codex.refused_turn();
                let mut steps: Vec<Step> = lines.into_iter().map(Step::Write).collect();
                // A refused `turn/start` is a turn that failed: the queue stops, and the person is told.
                if let Some(why) = refused {
                    self.turn_ended(agent_threads::drive::TurnOutcome::Failed);
                    steps.push(said(DriveEvent::Notice {
                        text: format!("Codex did not start the turn: {why}"),
                    }));
                }
                steps
            }
            // Codex did not start its thread: the program ends, and its end stops the thread.
            Err(fault) => {
                self.last_error = Some(fault.clone());
                vec![said(DriveEvent::Notice { text: fault }), Step::EndProgram]
            }
        }
    }

    fn program_closed(&mut self, id: &ProgramRequestId) -> Vec<Step> {
        let found: Vec<Uuid> = self
            .open
            .iter()
            .filter(|(_, open)| &open.request.id == id)
            .map(|(key, _)| *key)
            .collect();
        found
            .into_iter()
            .map(|key| {
                self.open.remove(&key);
                closed(key, "closed by the program")
            })
            .collect()
    }
}
