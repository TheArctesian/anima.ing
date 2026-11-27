//! The calls of the runner to one `codex app-server` (THR-021, TH10 Section 3): the start of the
//! server and of its thread, then a turn for each prompt. Pure: it gives the lines to write.

mod row;

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_threads::drive::{
    CodexDecoder, CodexInput, CodexPlace, CodexTurn, Effort, KeptFolder, Mode, Model, Prompt,
    TurnSettings,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a call of the runner was, so that its answer can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Initialize,
    ThreadStart,
    /// A `turn/start`; `leaves` when its policy takes the place of the profile that keeps the
    /// folder of Anima (THR-041), which holds only once Codex takes the turn.
    TurnStart {
        leaves: bool,
    },
    Other,
}

/// The state of the calls to one Codex server.
#[derive(Debug)]
pub struct Codex {
    pub decoder: CodexDecoder,
    folder: PathBuf,
    /// The folder of Anima in `folder`, which the sandbox keeps read-only (THR-041).
    kept: Option<KeptFolder>,
    /// The profile that keeps it is in force: no turn has sent a policy of its own yet.
    keeps: bool,
    mode: Mode,
    model: Option<Model>,
    effort: Option<Effort>,
    /// The speed that each turn carries, as the effort (TH117).
    speed: Option<String>,
    next: i64,
    calls: BTreeMap<i64, Call>,
    /// The ID of the thread of Codex, from its answer to `thread/start`.
    pub thread: Option<String>,
    /// The ID of the running turn, while one runs.
    pub turn: Option<String>,
    /// The turn that ended last, so that a late answer to `turn/start` does not open it again.
    ended: Option<String>,
    /// Prompts that wait for the thread of Codex.
    queued: Vec<Prompt>,
    /// Why Codex refused the last `turn/start`, until the thread reads it (TH109).
    refused_turn: Option<String>,
    /// What the person changed after the start (TH110).
    changed: Changed,
}

/// The model and the mode that the person changed after the start: each later turn carries them,
/// also after a restart of the runner (TH110). The effort goes with each turn already.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changed {
    pub model: Option<Model>,
    pub mode: Option<Mode>,
    /// The profile that keeps the folder of Anima read-only is still in force (THR-041). An
    /// older record has no answer: then the next turn sends its policy, which is never wider
    /// than the mode of the thread, where leaving it out could leave a wider one in force.
    #[serde(default)]
    pub keeps: bool,
}

impl Codex {
    #[must_use]
    pub fn new(folder: PathBuf, mode: Mode, model: Option<Model>, effort: Option<Effort>) -> Self {
        Self {
            decoder: CodexDecoder::new(),
            folder,
            kept: None,
            keeps: false,
            mode,
            model,
            effort,
            speed: None,
            next: 1,
            calls: BTreeMap::new(),
            thread: None,
            turn: None,
            ended: None,
            queued: Vec::new(),
            refused_turn: None,
            changed: Changed::default(),
        }
    }

    /// The folder of Anima that the folder of the thread holds: a start in the sandbox of the
    /// folder keeps it read-only (THR-041).
    #[must_use]
    pub fn keeping(mut self, kept: Option<KeptFolder>) -> Self {
        self.keeps = kept.is_some() && self.mode.folder_sandbox();
        self.kept = kept;
        self
    }

    /// A server that the holder kept while the daemon started again: its thread is known.
    #[must_use]
    pub fn known(mut self, thread: Option<String>, turn: Option<String>, changed: Changed) -> Self {
        self.thread = thread;
        self.turn = turn;
        // The record says whether a turn took the place of the profile; it can only narrow.
        self.keeps = self.keeps && changed.keeps;
        self.changed = changed;
        self.next = 1_000_000;
        self
    }

    fn call(&mut self, call: Call) -> i64 {
        let id = self.next;
        self.next += 1;
        self.calls.insert(id, call);
        id
    }

    /// The first line: `initialize`.
    pub fn begin(&mut self) -> String {
        let id = self.call(Call::Initialize);
        CodexInput::initialize(id)
    }

    /// A prompt: queued until the thread is there, a steer of the running turn, else a turn.
    pub fn prompt(&mut self, prompt: &Prompt) -> Vec<String> {
        let Some(thread) = self.thread.clone() else {
            self.queued.push(prompt.clone());
            return Vec::new();
        };
        let id = self.call(Call::Other);
        if let Some(turn) = self.turn.clone() {
            return vec![CodexInput::turn_steer(
                id,
                CodexTurn {
                    thread: &thread,
                    turn: &turn,
                },
                prompt,
            )];
        }
        // While a turn that leaves the profile waits for its answer, the profile may be gone:
        // this turn sends its policy too, never counting on the profile.
        let keeps = self.keeps && !self.leaving();
        let sends_policy = self
            .changed
            .mode
            .is_some_and(|mode| !(keeps && mode.folder_sandbox()));
        // A policy takes the place of the profile, for good, once Codex takes the turn.
        let leaves = self.keeps && sends_policy;
        self.calls.insert(id, Call::TurnStart { leaves });
        let turn = TurnSettings {
            model: self.changed.model.as_ref(),
            effort: self.effort,
            speed: self.speed.as_deref(),
            mode: self.changed.mode,
            keeps,
        };
        vec![CodexInput::turn_start(id, &thread, prompt, &turn)]
    }

    /// Asks for the skills of the folder again (TH117).
    pub fn skills_again(&mut self) -> String {
        let id = self.call(Call::Other);
        CodexInput::skills_list(id, &self.folder)
    }

    /// Compacts the context of the thread (TH117); `None` before Codex named its thread.
    pub fn compact(&mut self) -> Option<String> {
        let thread = self.thread.clone()?;
        let id = self.call(Call::Other);
        Some(CodexInput::thread_compact(id, &thread))
    }

    /// The speed of the next turns (TH117); `None` keeps the one they have.
    pub fn change_speed(&mut self, speed: Option<String>) {
        if speed.is_some() {
            self.speed = speed;
        }
    }

    /// The model, the effort or the mode of the next turns (TH110). A turn that runs keeps its own.
    pub fn change(&mut self, model: Option<Model>, effort: Option<Effort>, mode: Option<Mode>) {
        if model.is_some() {
            self.changed.model = model;
        }
        if effort.is_some() {
            self.effort = effort;
        }
        if mode.is_some() {
            self.changed.mode = mode;
        }
    }

    /// What the person changed, for the record.
    #[must_use]
    pub fn changed(&self) -> Changed {
        Changed {
            // A turn that leaves the profile and waits may be taken while the runner is down:
            // the record never counts on the profile then.
            keeps: self.keeps && !self.leaving(),
            ..self.changed.clone()
        }
    }

    /// Why Codex refused the last `turn/start`, once: that turn never runs, so it ended as failed.
    pub const fn refused_turn(&mut self) -> Option<String> {
        self.refused_turn.take()
    }

    /// A `turn/start` went and Codex did not answer it yet: that turn starts with what it carried.
    #[must_use]
    pub fn turn_starting(&self) -> bool {
        self.calls
            .values()
            .any(|call| matches!(call, Call::TurnStart { .. }))
    }

    /// Whether a prompt now goes into the running turn (`turn/steer`) and ends with it, not as a
    /// turn of its own.
    #[must_use]
    pub const fn steers(&self) -> bool {
        self.thread.is_some() && self.turn.is_some()
    }

    /// A turn started; Codex names it.
    pub fn turn_started(&mut self, turn: Option<String>) {
        self.taken();
        if turn.is_some() {
            self.turn = turn;
        }
    }

    /// The running turn ended: a `turn/start` that waits for its answer started it, so nothing
    /// waits for that answer any more.
    pub fn turn_ended(&mut self) {
        self.taken();
        self.ended = self.turn.take();
        self.calls
            .retain(|_, call| !matches!(call, Call::TurnStart { .. }));
    }

    /// Codex took the `turn/start` that waits: when its policy leaves the profile, the profile
    /// is gone (THR-041).
    fn taken(&mut self) {
        if self.leaving() {
            self.keeps = false;
        }
    }

    /// The ID of `line` when it is a `turn/start` whose policy leaves the profile.
    #[must_use]
    pub fn leaving_line(&self, line: &str) -> Option<i64> {
        let id = serde_json::from_str::<Value>(line).ok()?["id"].as_i64()?;
        matches!(self.calls.get(&id), Some(Call::TurnStart { leaves: true })).then_some(id)
    }

    /// A `turn/start` whose policy leaves the profile waits for its answer.
    #[must_use]
    pub fn leaving(&self) -> bool {
        self.calls
            .values()
            .any(|call| matches!(call, Call::TurnStart { leaves: true }))
    }

    /// `turn/interrupt` for the running turn, if one runs.
    pub fn interrupt(&mut self) -> Option<String> {
        let thread = self.thread.clone()?;
        let turn = self.turn.clone()?;
        let id = self.call(Call::Other);
        Some(CodexInput::turn_interrupt(
            id,
            CodexTurn {
                thread: &thread,
                turn: &turn,
            },
        ))
    }

    /// The answer to a call of the runner: the lines that follow it, or the text of a fault
    /// that ends the thread.
    ///
    /// # Errors
    /// The text of the fault when the server or its thread did not start.
    pub fn reply(&mut self, id: i64, ok: bool, result: &Value) -> Result<Vec<String>, String> {
        let call = self.calls.remove(&id).unwrap_or(Call::Other);
        if !ok {
            let message = result["message"].as_str().unwrap_or("no reason given");
            return match call {
                Call::Initialize | Call::ThreadStart => {
                    Err(format!("Codex did not start its thread: {message}"))
                }
                Call::TurnStart { .. } => {
                    self.refused_turn = Some(message.to_owned());
                    Ok(Vec::new())
                }
                Call::Other => Ok(Vec::new()),
            };
        }
        match call {
            Call::Initialize => {
                let id = self.call(Call::ThreadStart);
                let account = self.call(Call::Other);
                let limits = self.call(Call::Other);
                let models = self.call(Call::Other);
                let skills = self.call(Call::Other);
                Ok(vec![
                    CodexInput::initialized(),
                    CodexInput::thread_start(
                        id,
                        CodexPlace {
                            folder: &self.folder,
                            kept: self.kept.as_ref(),
                        },
                        self.mode,
                        self.model.as_ref(),
                    ),
                    // The account and its limits stay at the machine (THR-051, THR-132).
                    CodexInput::account_read(account),
                    CodexInput::account_limits_read(limits),
                    CodexInput::model_list(models),
                    CodexInput::skills_list(skills, &self.folder),
                ])
            }
            Call::ThreadStart => {
                let Some(thread) = result["thread"]["id"].as_str() else {
                    return Err("Codex gave no ID for its thread.".to_owned());
                };
                self.thread = Some(thread.to_owned());
                let queued = std::mem::take(&mut self.queued);
                Ok(queued
                    .iter()
                    .flat_map(|prompt| self.prompt(prompt))
                    .collect())
            }
            Call::TurnStart { leaves } => {
                self.keeps = self.keeps && !leaves;
                if let Some(turn) = result["turn"]["id"].as_str()
                    && self.ended.as_deref() != Some(turn)
                {
                    self.turn.get_or_insert_with(|| turn.to_owned());
                }
                Ok(Vec::new())
            }
            Call::Other => Ok(Vec::new()),
        }
    }
}
