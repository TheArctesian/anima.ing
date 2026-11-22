//! A change of the model, the effort or the mode of a thread after its start (TH110). Each takes
//! effect from the next turn. Pure.

use agent_threads::control::{Refusal, RunState};
use agent_threads::drive::{
    ClaudeInput, DriveEvent, Effort, Mode, Model, ProgramRequestId, ProgramState,
};

use super::{Driver, Step, Thread, said};

/// The first words of the ID of a control request of a change: a refusal of it is said.
const CHANGE_ID: &str = "anima-set-";

/// What the person changes; `None` keeps what the thread has.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub model: Option<Model>,
    pub effort: Option<Effort>,
    pub mode: Option<Mode>,
}

impl Thread {
    /// Changes the thread from its next turn. A call with a part that the program cannot change
    /// changes nothing.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended or the holder did not start it, and
    /// [`Refusal::CannotChange`] when the program cannot change a part after its start.
    pub fn set(&mut self, changes: Changes) -> Result<Vec<Step>, Refusal> {
        if self.row.state == RunState::Stopped || !self.held {
            return Err(Refusal::NotRunning);
        }
        let mut steps = Vec::new();
        match &mut self.driver {
            Driver::Codex(codex) => {
                if changes.mode == Some(Mode::Plan) {
                    return Err(cannot(
                        "Codex has no plan mode that the runner can set: choose a different mode.",
                    ));
                }
                codex.change(changes.model.clone(), changes.effort, changes.mode);
            }
            Driver::Claude(_) => {
                if changes.effort.is_some() {
                    return Err(cannot(
                        "Claude Code cannot change the effort of a session that runs: start a new thread with the effort you want.",
                    ));
                }
                let lines = self.claude_lines(&changes);
                self.changes.extend(lines);
                steps.extend(self.waiting_changes());
            }
        }
        self.note_changes(changes);
        steps.push(self.row_change());
        Ok(steps)
    }

    fn claude_lines(&mut self, changes: &Changes) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(model) = &changes.model {
            self.controls += 1;
            lines.push(ClaudeInput::set_model(
                &format!("{CHANGE_ID}model-{}", self.controls),
                model,
            ));
        }
        if let Some(mode) = changes.mode {
            self.controls += 1;
            lines.push(ClaudeInput::set_permission_mode(
                &format!("{CHANGE_ID}mode-{}", self.controls),
                mode,
            ));
        }
        lines
    }

    fn note_changes(&mut self, changes: Changes) {
        if let Some(model) = changes.model {
            self.row.model = Some(model.as_str().to_owned());
        }
        if let Some(effort) = changes.effort {
            self.row.effort = Some(effort);
        }
        if let Some(mode) = changes.mode {
            self.row.mode = mode;
        }
    }

    /// The control requests of a change, once no turn runs or waits: Claude Code was measured to
    /// take them between two turns only.
    pub(super) fn waiting_changes(&mut self) -> Vec<Step> {
        let between = self.program == Some(ProgramState::Idle) && self.pending == 0;
        if !between {
            return Vec::new();
        }
        self.changes.drain(..).map(Step::Write).collect()
    }

    /// A refusal of Claude Code to change the model or the mode, said to the person.
    pub(super) fn change_refused(id: &ProgramRequestId, ok: bool) -> Vec<Step> {
        let ProgramRequestId::Text(id) = id else {
            return Vec::new();
        };
        if ok || !id.starts_with(CHANGE_ID) {
            return Vec::new();
        }
        let what = if id.contains("model") {
            "model"
        } else {
            "mode"
        };
        vec![said(DriveEvent::Notice {
            text: format!("Claude Code did not change the {what} of the thread."),
        })]
    }
}

fn cannot(said: &str) -> Refusal {
    Refusal::CannotChange {
        said: said.to_owned(),
    }
}
