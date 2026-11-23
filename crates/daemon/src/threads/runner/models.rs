//! The models of a program before any of its threads started (TH104): the runner starts the
//! program by itself, asks only for its list, and ends it. The page of a new thread shows its
//! pickers from it. Measured on 2026-11-22: Claude Code 2.x answers `initialize` with no prompt,
//! with no session file and no hook; `codex app-server` 0.170 answers `model/list` with no thread.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent_threads::control::{ListState, Outcome, ProgramState};
use agent_threads::drive::{
    ClaudeInput, CodexInput, DriveEvent, ModelChoice, Program, claude_models, codex_models,
};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::socket::Caller;
use super::{Runner, login};

pub(super) mod asking;
use asking::{Fault, Heard};

/// The list of a program, or why the ask gave none.
pub(super) type Listed = Result<Vec<ModelChoice>, Fault>;

/// How long a program that gave no list is not asked again.
const ASK_AGAIN: Duration = Duration::from_secs(60);
/// The most calls that wait for the asks; a call past it gets the lists known now.
const MOST_WAITING: usize = 64;

/// The asks that run, and the calls that wait for them.
pub(super) struct Asks {
    pub sender: tokio::sync::mpsc::Sender<(Program, Listed)>,
    pub receiver: Option<tokio::sync::mpsc::Receiver<(Program, Listed)>>,
    /// The programs being asked.
    open: Vec<Program>,
    /// When each program last gave no list, and why.
    failed: Vec<(Program, Instant, Fault)>,
    /// When the running ask or the last failed ask of each program started or ended, for the
    /// person (TH117).
    since: Vec<(Program, DateTime<Utc>)>,
    /// The programs that are not on this computer, as the last call of the person found.
    missing: Vec<Program>,
    waiting: Vec<Caller>,
}

impl Default for Asks {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(4);
        Self {
            sender,
            receiver: Some(receiver),
            open: Vec::new(),
            failed: Vec::new(),
            since: Vec::new(),
            missing: Vec::new(),
            waiting: Vec::new(),
        }
    }
}

impl Runner {
    /// The call `models`: at once when each program of this computer has a list, else when the
    /// asks for the missing lists end. Only a call of the person starts a program: a process of an
    /// agent gets the lists known now (R-315).
    pub(super) fn models_call(&mut self, back: Caller, of_the_person: bool) {
        let known: Vec<Program> = self
            .said
            .models
            .rows()
            .iter()
            .map(|row| row.program)
            .collect();
        for program in [Program::ClaudeCode, Program::Codex] {
            let failed = self
                .model_asks
                .failed
                .iter()
                .any(|(failed, at, _)| *failed == program && at.elapsed() < ASK_AGAIN);
            let asked = known.contains(&program) || self.model_asks.open.contains(&program);
            if !of_the_person || asked || failed {
                continue;
            }
            let found = login::find(program, &self.env, login::executable, login::trusted);
            self.model_asks
                .missing
                .retain(|missing| *missing != program);
            let Some(path) = found else {
                self.model_asks.missing.push(program);
                continue;
            };
            self.model_asks.noted(program);
            let env = environment(program, self.env.clone(), &path);
            let sender = self.model_asks.sender.clone();
            self.model_asks.open.push(program);
            tokio::spawn(async move {
                let models = ask(program, path, env).await;
                if let Err(fault) = models {
                    let why = fault.words();
                    tracing::info!(?program, why, "a program gave no list of its models");
                }
                // A runner that ended does not need the answer.
                let _ = sender.send((program, models)).await;
            });
        }
        if self.model_asks.open.is_empty() || self.model_asks.waiting.len() >= MOST_WAITING {
            back.answer(self.models_outcome());
        } else {
            self.model_asks.waiting.push(back);
        }
    }

    /// The end of one ask: its list is kept, and when no ask runs each waiting call gets the
    /// lists.
    pub(super) fn models_asked(&mut self, program: Program, models: Listed) {
        self.model_asks.open.retain(|open| *open != program);
        self.model_asks
            .failed
            .retain(|(failed, _, _)| *failed != program);
        match models {
            Ok(models) => self.said.models.note(program, models, Utc::now()),
            Err(fault) => {
                self.model_asks
                    .failed
                    .push((program, Instant::now(), fault));
                self.model_asks.noted(program);
            }
        }
        if self.model_asks.open.is_empty() {
            let outcome = self.models_outcome();
            for back in self.model_asks.waiting.drain(..) {
                back.answer(outcome.clone());
            }
        }
    }

    fn models_outcome(&self) -> Outcome {
        let models = self.said.models.rows();
        let asks = &self.model_asks;
        let programs = [Program::ClaudeCode, Program::Codex]
            .into_iter()
            .map(|program| {
                let state = if models.iter().any(|row| row.program == program) {
                    ListState::Listed
                } else if asks.open.contains(&program) {
                    ListState::Asking
                } else if asks.failed.iter().any(|(failed, _, _)| *failed == program) {
                    ListState::Failed
                } else if asks.missing.contains(&program) {
                    ListState::Missing
                } else {
                    ListState::NotAsked
                };
                let since = matches!(state, ListState::Asking | ListState::Failed)
                    .then(|| asks.since.iter().find(|(of, _)| *of == program))
                    .flatten()
                    .map(|(_, at)| *at);
                let reason = asks
                    .failed
                    .iter()
                    .find(|(failed, _, _)| *failed == program && state == ListState::Failed)
                    .map(|(_, _, fault)| fault.words().to_owned());
                ProgramState {
                    program,
                    state,
                    since,
                    reason,
                }
            })
            .collect();
        Outcome::Models { models, programs }
    }
}

impl Asks {
    /// Now is the time of the newest ask of `program`, or of its failure.
    fn noted(&mut self, program: Program) {
        self.since.retain(|(of, _)| *of != program);
        self.since.push((program, Utc::now()));
    }
}

/// The environment of a start of the program, as a thread gets it (THR-040).
pub(super) fn environment(
    program: Program,
    env: Vec<(String, String)>,
    path: &std::path::Path,
) -> Vec<(String, String)> {
    let mut env = login::with_folder_of(env, path);
    if program == Program::ClaudeCode {
        env.extend(
            login::CLAUDE_VARIABLES.map(|(name, value)| (name.to_owned(), value.to_owned())),
        );
    }
    env
}

/// Asks the program for its list of models: `None` when it gives none.
async fn ask(program: Program, path: PathBuf, env: Vec<(String, String)>) -> Listed {
    let first = match program {
        Program::ClaudeCode => ClaudeInput::initialize("models"),
        Program::Codex => CodexInput::initialize(1),
    };
    let asked = asking::Asked { program, path, env };
    asking::ask(asked, first, |value| {
        match (program, list_in(program, value)) {
            (_, Some(models)) => Heard::Done(models),
            (Program::Codex, None) if asking::initialized(value) => {
                Heard::Write(vec![CodexInput::initialized(), CodexInput::model_list(2)])
            }
            _ => Heard::Nothing,
        }
    })
    .await
}

/// The list in one line of the program, when the line is the answer that holds it.
fn list_in(program: Program, value: &Value) -> Option<Vec<ModelChoice>> {
    let event = match program {
        Program::ClaudeCode
            if value["type"] == "control_response"
                && value["response"]["request_id"] == "models" =>
        {
            claude_models(&value["response"]["response"])
        }
        Program::Codex if value["id"] == 2 => codex_models(&value["result"]),
        _ => None,
    };
    match event {
        Some(DriveEvent::Models { models }) => Some(models),
        _ => None,
    }
}
