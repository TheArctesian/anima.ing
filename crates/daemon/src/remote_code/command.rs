//! Holder-side K10 receipts. An ambiguous handoff is retained without repeating its effect.

mod control;
mod offer;
mod receipts;
pub use offer::{
    Action, Answer, Choice, Decision, Offer, Question, Request, Rule, Selection, Target, Value,
};

use agent_threads::drive::Mode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{launch::ProcessIdentity, wire::Owner};

/// K10 command identity is an opaque operator-supplied string, never a file name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "String")]
pub struct CommandId(String);

impl TryFrom<String> for CommandId {
    type Error = &'static str;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        if !(1..=256).contains(&id.len()) || id.chars().any(char::is_control) {
            return Err("invalid command ID");
        }
        Ok(Self(id))
    }
}

pub enum Admission {
    Effect(Admitted),
    Existing(Consumption),
}

/// One persisted, unresolved provider handoff.
pub struct Admitted {
    id: CommandId,
    fingerprint: [u8; 32],
    end: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivered {
    pub id: CommandId,
    #[serde(flatten)]
    pub owner: Owner,
    pub launch: Uuid,
    pub thread: Uuid,
    pub payload: Payload,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payload {
    Prompt {
        prompt: String,
    },
    Answer {
        process: String,
        revision: u64,
        request: String,
        answer: Answer,
    },
    Stop {
        target: Target,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    ThreadUnknown,
    ThreadEnded,
    ThreadsOff,
    FolderNotPermitted,
    RequestUnknown,
    RequestResolved,
    RequestExpired,
    Busy,
    AccessChanged,
    TargetStale,
    InvalidAnswer,
    CancelledByStop,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReceiptOutcome {
    Recorded { cause: () },
    Refused { cause: Cause },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Receipt {
    pub id: CommandId,
    #[serde(flatten)]
    pub outcome: ReceiptOutcome,
}

pub enum Handoff {
    Settled(ReceiptOutcome),
    Uncertain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Consumption {
    Settled(Receipt),
    Reconciling,
}

/// The runner constructs this snapshot while serializing with local changes and answers.
/// The offer and process must describe the current holder process, even after a daemon restart.
pub struct LiveControl<'a> {
    pub process: &'a ProcessIdentity,
    pub offer: &'a Offer,
    pub mode: Mode,
    pub ceiling: Mode,
    pub threads_on: bool,
    pub folder_permitted: bool,
    pub publishable: bool,
}

#[derive(Deserialize, Serialize)]
struct RecordedBinding {
    launch: Uuid,
    thread: Uuid,
}

#[derive(Deserialize, Serialize)]
pub(super) struct SavedCommand {
    binding: RecordedBinding,
    fingerprint: [u8; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request: Option<[u8; 32]>,
    state: CommandState,
}

#[derive(Deserialize, Serialize)]
enum CommandState {
    HandingOff { stop: bool },
    Settled(ReceiptOutcome),
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct SavedOffer {
    revision: u64,
    hash: [u8; 32],
}
