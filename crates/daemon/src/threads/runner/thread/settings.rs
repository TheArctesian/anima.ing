//! A change of the model, the effort, the speed or the mode of a thread after its start (TH110,
//! TH117). The row
//! says a change only when the program has it, so the person never sees a mode that the program
//! does not run in. Pure.

use std::collections::BTreeMap;

use agent_threads::control::{Refusal, RunState};
use agent_threads::drive::{ClaudeInput, DriveEvent, Effort, Mode, Model, ProgramRequestId};
use serde::{Deserialize, Serialize};

use super::super::accounts::STANDARD;
use super::{Driver, Step, Thread, said};

/// The first words of the ID of a control request of a change.
const CHANGE_ID: &str = "anima-set-";

/// What the person changes; `None` keeps what the thread has.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub model: Option<Model>,
    pub effort: Option<Effort>,
    pub mode: Option<Mode>,
    /// A speed of the model, or `default` (TH117). Codex only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
}

impl Changes {
    /// These changes, then `later` over them.
    fn and(self, later: Self) -> Self {
        Self {
            model: later.model.or(self.model),
            effort: later.effort.or(self.effort),
            mode: later.mode.or(self.mode),
            speed: later.speed.or(self.speed),
        }
    }
}

/// The changes that the person asked and the program does not have yet. The record keeps them,
/// so that an answer of Claude Code after a restart of the runner still changes the row.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Asked {
    /// Claude Code: each control request by its ID, until Claude Code answers it.
    claude: BTreeMap<String, Changes>,
    /// Codex: the changes for the turns after the running one.
    codex: Option<Changes>,
    /// The number of the last control request of a change, kept in the record so that an ID is
    /// never made two times.
    last: u64,
}

impl Thread {
    /// Changes the thread. Claude Code gets its control requests at once and the row changes
    /// when it answers that it took them; Codex takes a change with its next turn, and the row
    /// changes when the running turn ended. A call with a part that the program cannot change
    /// changes nothing.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended or the holder did not start it, and
    /// [`Refusal::CannotChange`] when the program cannot change a part after its start.
    pub fn set(&mut self, changes: Changes) -> Result<Vec<Step>, Refusal> {
        if self.row.state == RunState::Stopped || !self.held {
            return Err(Refusal::NotRunning);
        }
        match &self.driver {
            Driver::Codex(_) if changes.mode == Some(Mode::Plan) => Err(cannot(
                "Codex has no plan mode that the runner can set: choose a different mode.",
            )),
            Driver::Codex(_) => {
                let waiting = self.asked.codex.take().unwrap_or_default();
                self.asked.codex = Some(waiting.and(changes));
                Ok(self.codex_landed())
            }
            Driver::Claude(_) if changes.effort.is_some() => Err(cannot(
                "Claude Code cannot change the effort of a session that runs: start a new thread with the effort you want.",
            )),
            Driver::Claude(_) if changes.speed.is_some() => {
                Err(cannot("Claude Code has no speed that the runner can set."))
            }
            Driver::Claude(_) => Ok(self.claude_lines(changes)),
        }
    }

    /// One control request for each part; Claude Code was measured to take them between two
    /// turns, and its answer says whether it took them.
    fn claude_lines(&mut self, changes: Changes) -> Vec<Step> {
        let mut steps = Vec::new();
        if let Some(model) = changes.model {
            self.asked.last += 1;
            let id = format!("{CHANGE_ID}model-{}", self.asked.last);
            steps.push(Step::Write(ClaudeInput::set_model(&id, &model)));
            let part = Changes {
                model: Some(model),
                ..Changes::default()
            };
            self.asked.claude.insert(id, part);
        }
        if let Some(mode) = changes.mode {
            self.asked.last += 1;
            let id = format!("{CHANGE_ID}mode-{}", self.asked.last);
            steps.push(Step::Write(ClaudeInput::set_permission_mode(&id, mode)));
            let part = Changes {
                mode: Some(mode),
                ..Changes::default()
            };
            self.asked.claude.insert(id, part);
        }
        steps
    }

    /// The changes of Codex land when no turn runs: the next `turn/start` carries them.
    pub(super) fn codex_landed(&mut self) -> Vec<Step> {
        let Driver::Codex(codex) = &mut self.driver else {
            return Vec::new();
        };
        if codex.turn.is_some() || codex.turn_starting() {
            return Vec::new();
        }
        let Some(changes) = self.asked.codex.take() else {
            return Vec::new();
        };
        codex.change(changes.model.clone(), changes.effort, changes.mode);
        codex.change_speed(changes.speed.clone());
        self.note_changes(changes);
        vec![self.row_change()]
    }

    /// The answer of Claude Code to a control request of a change: the row says it when Claude
    /// Code took it, and the person is told when it did not.
    pub(super) fn change_answered(&mut self, id: &ProgramRequestId, ok: bool) -> Vec<Step> {
        let ProgramRequestId::Text(id) = id else {
            return Vec::new();
        };
        let Some(changes) = self.asked.claude.remove(id) else {
            return Vec::new();
        };
        if ok {
            self.note_changes(changes);
            return vec![self.row_change()];
        }
        let what = if changes.model.is_some() {
            "model"
        } else {
            "mode"
        };
        vec![said(DriveEvent::Notice {
            text: format!(
                "Claude Code did not change the {what} of the thread; it keeps the one it had."
            ),
        })]
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
        if let Some(speed) = changes.speed {
            // The standard speed is no speed of the row: a start again sends none.
            self.row.speed = Some(speed).filter(|speed| speed != STANDARD);
        }
    }
}

fn cannot(said: &str) -> Refusal {
    Refusal::CannotChange {
        said: said.to_owned(),
    }
}
