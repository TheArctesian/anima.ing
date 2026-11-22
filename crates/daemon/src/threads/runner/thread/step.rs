//! The steps that the core of a thread gives the runner: commands to the holder, and what the
//! runner does itself. Pure.

use agent_threads::control::{Change, Queued};
use agent_threads::drive::{AccountFacts, ModelChoice};

use crate::threads::wire::Action;

/// Something the runner does for a thread, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// A line to the standard input of the program.
    Write(String),
    /// Keep the processes below the program now as spared (the MCP servers of Codex).
    Mark,
    /// End the processes below the program that are not spared (the stop of a Codex turn).
    EndChildren,
    /// End the program: it cannot go on (Codex did not start its thread).
    EndProgram,
    /// A line of the log of the runner, of the kind "stop", for a stop that the runner made.
    Log(&'static str),
    /// An update for the clients.
    Change(Change),
    /// What the program said of its account: for the runner only, never an update (THR-051).
    Account(Box<AccountFacts>),
    /// The models that the program offers (TH104): for the runner, not an update.
    Models(Vec<ModelChoice>),
    /// A prompt of the queue whose lines are the writes before this step: it is shown as sent
    /// when the holder took its last line, and waits again in the queue when not (TH109).
    Queued(Queued),
}

impl Step {
    /// The command to the holder that this step is, or `None` when the runner does it itself.
    #[must_use]
    pub fn command(&self, thread: &str) -> Option<Action> {
        let thread = thread.to_owned();
        Some(match self {
            Self::Write(line) => Action::Write {
                thread,
                line: line.clone(),
            },
            Self::Mark => Action::Mark { thread },
            Self::EndChildren => Action::EndChildren { thread },
            Self::EndProgram => Action::Stop { thread },
            Self::Log(_)
            | Self::Change(_)
            | Self::Account(_)
            | Self::Models(_)
            | Self::Queued(_) => return None,
        })
    }
}
