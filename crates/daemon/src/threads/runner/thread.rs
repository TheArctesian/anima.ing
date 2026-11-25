//! One thread of the runner: what its program said, the requests that wait for the person, and
//! its state by the order of trust of THR-085 (the end of the process, then the stream of the
//! program). Pure: each call gives the [`Step`]s that the runner does, in order.

use std::collections::{BTreeMap, VecDeque};

use agent_threads::control::{Change, Refusal, RunState, ThreadRow, Update};
use agent_threads::drive::{
    ClaudeDecoder, ClaudeInput, DriveEvent, Effort, Program, ProgramState, Prompt, Request,
};
use agent_threads::first_line;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::codex::{Changed, Codex};

mod asks;
mod commands;
mod events;
mod kept;
mod queue;
mod record;
mod remote_code;
mod settings;
mod step;

pub use settings::Changes;
pub use step::Step;

/// How many updates of a thread the runner keeps for a client that starts to watch, and their
/// most bytes as JSON: a line of a program can be 4 MiB, so a count alone does not bound them.
const KEPT: usize = 4000;
const KEPT_BYTES: usize = 4 * 1024 * 1024;

/// The source of a prompt that came through the local socket (THR-042).
pub const THIS_COMPUTER: &str = "this computer";

/// A prompt as the clients see it: its words and what went with it (TH111), never the bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub text: String,
    pub images: u32,
    pub files: Vec<String>,
}

impl Shown {
    #[must_use]
    pub fn of(prompt: &Prompt) -> Self {
        Self {
            text: prompt.text.clone(),
            images: prompt.images(),
            files: prompt.files(),
        }
    }
}

/// A request that waits for the person, with the line it came in, so that a runner that starts
/// again reads it again (THR-030).
#[derive(Clone, Debug)]
pub struct Open {
    pub request: Request,
    pub line: String,
    /// An answer was given and waits for the holder to take it; a second one is refused.
    pub answering: bool,
}

#[derive(Debug)]
enum Driver {
    Claude(ClaudeDecoder),
    Codex(Box<Codex>),
    Grok(Box<super::grok::Grok>),
}

/// What the runner writes to its record for a thread.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Saved {
    pub row: ThreadRow,
    pub effort: Option<Effort>,
    pub codex_thread: Option<String>,
    pub codex_turn: Option<String>,
    #[serde(default)]
    pub grok: Option<super::grok::Saved>,
    /// What the person changed of a Codex thread after its start (TH110).
    #[serde(default)]
    pub codex_changed: Changed,
    /// The changes of the person that the program did not take yet (TH110).
    #[serde(default)]
    pub asked: settings::Asked,
    /// The open requests: the ID of the runner and the line of the program.
    pub open: Vec<(Uuid, String)>,
    /// The open requests whose answer went to the holder: after a restart they close, so that
    /// the program never gets a second answer (THR-030).
    #[serde(default)]
    pub answering: Vec<Uuid>,
    /// The number of the last update, so that a client that watches across a restart of the
    /// runner never takes a new update for an old one.
    #[serde(default)]
    pub seq: u64,
}

/// One thread.
#[derive(Debug)]
pub struct Thread {
    pub row: ThreadRow,
    driver: Driver,
    open: BTreeMap<Uuid, Open>,
    program: Option<ProgramState>,
    seq: u64,
    kept: VecDeque<(Update, usize)>,
    kept_bytes: usize,
    controls: u64,
    /// The holder started the program: a line may be written to it.
    held: bool,
    last_error: Option<String>,
    /// The queue stopped after an interrupt or a failed turn, until the person sends (TH109).
    stalled: bool,
    /// The prompts that went as turns of their own and whose turns did not end yet: a queued
    /// prompt goes only at none, also when the program says that it is idle in between. Claude
    /// Code holds a prompt sent during a turn and runs it as a turn of its own (THR-020), so each
    /// counts; a Codex steer ends with the running turn (TH109).
    pending: u32,
    /// The changes of the person that the program does not have yet (TH110).
    asked: settings::Asked,
    /// The commands and the skills that the program last named (TH117). In memory only.
    lists: Option<agent_threads::drive::CommandLists>,
    /// When a call last asked Codex for its skills again (TH117).
    lists_asked: Option<std::time::Instant>,
}

impl Thread {
    #[must_use]
    pub fn new(mut row: ThreadRow, effort: Option<Effort>, codex: Option<Codex>) -> Self {
        // A record of an older runner keeps the effort beside the row.
        row.effort = row.effort.or(effort);
        let driver = match codex {
            Some(codex) => Driver::Codex(Box::new(codex)),
            None if row.program == Program::Grok => {
                Driver::Grok(Box::new(super::grok::Grok::new(&row)))
            }
            None => Driver::Claude(ClaudeDecoder::new()),
        };
        Self {
            row,
            driver,
            open: BTreeMap::new(),
            program: None,
            seq: 0,
            kept: VecDeque::new(),
            kept_bytes: 0,
            controls: 0,
            held: false,
            last_error: None,
            stalled: false,
            pending: 0,
            asked: settings::Asked::default(),
            lists: None,
            lists_asked: None,
        }
    }

    fn decode(&mut self, line: &[u8]) -> Vec<DriveEvent> {
        match &mut self.driver {
            Driver::Grok(grok) => grok.read(line),
            Driver::Claude(decoder) => decoder.read(line),
            Driver::Codex(codex) => codex.decoder.read(line),
        }
    }

    /// The first steps of a new thread: its prompt, and the first line of the program.
    pub fn begin(&mut self, prompt: &str) -> Vec<Step> {
        self.begin_with(&Prompt::plain(prompt))
    }

    /// The first steps of a new thread whose first prompt has files (TH111).
    pub fn begin_with(&mut self, prompt: &Prompt) -> Vec<Step> {
        self.held = true;
        self.pending = 1;
        self.row.title = first_line(&prompt.text, agent_threads::TITLE_LENGTH);
        let mut steps = Self::sent(&Shown::of(prompt));
        match &mut self.driver {
            Driver::Grok(grok) => steps.push(Step::Write(grok.begin(prompt))),
            Driver::Claude(_) => {
                // Its answer names the models of the program (TH104).
                steps.push(Step::Write(ClaudeInput::initialize("anima-initialize")));
                steps.push(Step::Write(ClaudeInput::prompt_with(prompt)));
            }
            Driver::Codex(codex) => {
                steps.push(Step::Write(codex.begin()));
                codex.prompt(prompt);
            }
        }
        steps
    }

    /// Initialize the provider without submitting a user turn.
    pub(super) fn begin_remote(&mut self) -> Vec<Step> {
        self.held = true;
        match &mut self.driver {
            Driver::Claude(_) => {
                self.program = Some(ProgramState::Idle);
                self.row.state = RunState::Idle;
                vec![Step::Write(ClaudeInput::initialize("anima-initialize"))]
            }
            Driver::Codex(codex) => vec![Step::Write(codex.begin())],
            Driver::Grok(_) => Vec::new(),
        }
    }

    /// The lines of a prompt of the person to a running thread. The prompt is shown as sent
    /// only when the holder took them ([`Self::sent`]); a Codex prompt before its thread has an
    /// ID waits in the thread and has no line yet.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program ended or the holder did not start it yet.
    pub fn send(&mut self, prompt: &Prompt, person: bool) -> Result<Vec<String>, Refusal> {
        if self.row.state == RunState::Stopped || !self.held {
            return Err(Refusal::NotRunning);
        }
        if matches!(&self.driver, Driver::Grok(grok) if !grok.saved.ready || grok.saved.turn.is_some() || !self.asked.claude.is_empty())
        {
            return Err(Refusal::CannotChange {
                said: "Grok is working. Queue this prompt or stop the current turn first.".into(),
            });
        }
        // A prompt of the person starts the queue again after the turn of it; a prompt of an
        // agent does not (TH109).
        if person {
            self.stalled = false;
        }
        // Each prompt is a turn that must end before the queue goes on, but a steer of a running
        // Codex turn. The state of the program is not read here: it says Idle until the program
        // says that the new turn runs, so a second prompt in that moment would not count.
        let steers = matches!(&self.driver, Driver::Codex(codex) if codex.steers());
        if !steers || self.pending == 0 {
            self.pending = self.pending.saturating_add(1);
        }
        Ok(match &mut self.driver {
            Driver::Grok(grok) => {
                self.program = Some(ProgramState::Working);
                grok.prompt(prompt)
            }
            Driver::Claude(_) => vec![ClaudeInput::prompt_with(prompt)],
            Driver::Codex(codex) => codex.prompt(prompt),
        })
    }

    /// The holder took the lines of a prompt: it is shown with its source (THR-042) and its
    /// files (TH111).
    #[must_use]
    pub fn sent(shown: &Shown) -> Vec<Step> {
        vec![Step::Change(Change::Prompt {
            text: shown.text.clone(),
            source: THIS_COMPUTER.to_owned(),
            images: shown.images,
            files: shown.files.clone(),
        })]
    }

    pub fn sent_to(&mut self, shown: &Shown) -> Vec<Step> {
        let mut steps = Self::sent(shown);
        steps.extend(self.state_change());
        steps
    }

    /// The person stops the running turn; the program stays. Each open request closes (THR-087:
    /// a stop by the person).
    pub fn interrupt(&mut self) -> Vec<Step> {
        self.stall();
        let mut steps = self.turn_stop();
        steps.extend(self.close_all("the turn was stopped"));
        steps.extend(self.state_change());
        steps
    }

    /// The lines that stop the running turn. Codex (TH10) and Claude Code (seen on a Mac on
    /// 2026-11-21: a `sleep` of the Bash tool ran on) leave the command of a turn alive after
    /// their interrupt, so the processes below the program that the turn started end too
    /// (THR-049); its MCP servers, marked before the turn, are spared. With no running turn of
    /// Codex there is nothing to stop.
    fn turn_stop(&mut self) -> Vec<Step> {
        match &mut self.driver {
            Driver::Grok(grok) => {
                if !grok.saved.ready {
                    return vec![Step::EndProgram];
                }
                let mut steps: Vec<Step> = grok.cancel().into_iter().map(Step::Write).collect();
                steps.push(Step::EndChildren);
                steps
            }
            Driver::Claude(_) => {
                self.controls += 1;
                let id = format!("anima-interrupt-{}", self.controls);
                vec![Step::Write(ClaudeInput::interrupt(&id)), Step::EndChildren]
            }
            Driver::Codex(codex) => codex
                .interrupt()
                .map_or_else(Vec::new, |line| vec![Step::Write(line), Step::EndChildren]),
        }
    }

    /// The program ended: each open request closes and the thread is stopped (THR-030, THR-085).
    pub fn exited(&mut self, code: Option<i32>, signal: Option<i32>) -> Vec<Step> {
        let mut steps = self.close_all("the process of the program ended");
        let why = match (code, signal) {
            (Some(code), _) if code != 0 => format!("The program ended with code {code}."),
            (None, Some(signal)) => format!("The program was ended by signal {signal}."),
            _ => "The program ended.".to_owned(),
        };
        let why = match &self.last_error {
            Some(error) if code != Some(0) => format!("{why} It said: {error}"),
            _ => why,
        };
        self.row.ended = Some(why);
        self.row.state = RunState::Stopped;
        // A stop also stops the queue (THR-102).
        self.row.queued.clear();
        self.row.open.clear();
        steps.push(Step::Change(Change::State {
            state: RunState::Stopped,
            row: Box::new(self.row.clone()),
        }));
        steps
    }

    /// The thread was not running when the runner started: its holder ended (THR-049).
    pub fn lost(&mut self) -> Vec<Step> {
        self.ended_because("The holder of the program ended; the thread stopped.".to_owned())
    }

    /// The program ended, for a reason that the runner knows better than the exit code.
    pub fn ended_because(&mut self, why: String) -> Vec<Step> {
        let mut steps = self.exited(None, None);
        self.row.ended = Some(why);
        if let Some(Step::Change(Change::State { row, .. })) = steps.last_mut() {
            row.ended.clone_from(&self.row.ended);
        }
        steps
    }

    /// One line of standard error: kept as the last thing the program said of a fault.
    pub fn error_line(&mut self, text: &str) {
        let text = first_line(text, 300);
        if !text.is_empty() {
            self.last_error = Some(text);
        }
    }

    /// The state now: an open request first (sticky, THR-086), then what the program said.
    fn state(&self) -> RunState {
        if self.row.state == RunState::Stopped {
            return RunState::Stopped;
        }
        if !self.open.is_empty() {
            return RunState::NeedsYou;
        }
        match self.program {
            None => RunState::Starting,
            Some(ProgramState::Working | ProgramState::NeedsAnswer) => RunState::Working,
            Some(ProgramState::Idle) => RunState::Idle,
        }
    }

    /// A change of the state, when the state or the open requests changed.
    fn state_change(&mut self) -> Option<Step> {
        let state = self.state();
        let open: Vec<Uuid> = self.open.keys().copied().collect();
        if state == self.row.state && open == self.row.open {
            return None;
        }
        self.row.state = state;
        self.row.open = open;
        Some(Step::Change(Change::State {
            state,
            row: Box::new(self.row.clone()),
        }))
    }

    /// The program of the thread.
    #[must_use]
    pub const fn program(&self) -> Program {
        self.row.program
    }
}

fn said(event: DriveEvent) -> Step {
    Step::Change(Change::Said { event })
}

fn closed(request: Uuid, why: &str) -> Step {
    Step::Change(Change::Closed {
        request,
        why: why.to_owned(),
    })
}

fn prompt_change(text: &str) -> Step {
    Step::Change(Change::Prompt {
        text: text.to_owned(),
        source: THIS_COMPUTER.to_owned(),
        images: 0,
        files: Vec::new(),
    })
}
