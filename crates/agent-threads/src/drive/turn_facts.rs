//! What a program says of a thread beside its words: the diff and the plan of a Codex turn
//! (TH101, TH102) and the use of tokens (TH103). The models are in `lists/models.rs`.
//! Pure: each function reads one JSON value. None of these facts names the account (THR-051).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DriveEvent, text_of};
use crate::{MAX_TEXT, held};

/// The most steps of one plan that a decoder keeps.
const MAX_STEPS: usize = 100;
/// The most bytes of one step.
const MAX_WORDS: usize = 1024;

/// One step of the plan of a turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub step: String,
    pub state: StepState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Pending,
    InProgress,
    Completed,
}

/// The use of tokens of a thread, as the program last said it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// The tokens in the context of the model now: what the next request sends.
    pub context: Option<u64>,
    /// The size of the context window of the model.
    pub window: Option<u64>,
    /// The input tokens (with the tokens read from or written to a cache) and the output
    /// tokens: of the thread for Codex, of the last turn for Claude Code.
    pub input: u64,
    pub output: u64,
    /// What Claude Code says the turn cost, in US dollars at list price; Codex says no cost.
    pub cost_usd: Option<f64>,
}

/// `turn/diff/updated` of Codex: the whole diff of the turn so far.
pub fn codex_diff(params: &Value) -> DriveEvent {
    DriveEvent::TurnDiff {
        turn: params["turnId"].as_str().map(str::to_owned),
        diff: held(&text_of(&params["diff"]), MAX_TEXT),
    }
}

/// `turn/plan/updated` of Codex. A step whose state is not known is pending.
pub fn codex_plan(params: &Value) -> DriveEvent {
    let steps = list(&params["plan"], MAX_STEPS)
        .map(|step| PlanStep {
            step: held(&text_of(&step["step"]), MAX_WORDS),
            state: match step["status"].as_str() {
                Some("inProgress") => StepState::InProgress,
                Some("completed") => StepState::Completed,
                _ => StepState::Pending,
            },
        })
        .collect();
    DriveEvent::Plan {
        explanation: params["explanation"]
            .as_str()
            .map(|text| held(text, MAX_TEXT)),
        steps,
    }
}

/// `thread/tokenUsage/updated` of Codex: `total` is the thread, `last` the last request.
pub fn codex_usage(params: &Value) -> DriveEvent {
    let usage = &params["tokenUsage"];
    DriveEvent::Usage {
        usage: Usage {
            context: usage["last"]["totalTokens"].as_u64(),
            window: usage["modelContextWindow"].as_u64(),
            input: number(&usage["total"]["inputTokens"]),
            output: number(&usage["total"]["outputTokens"]),
            cost_usd: None,
        },
    }
}

/// The use of the turn from the `result` line of Claude Code. The context now is the last
/// request of the turn (its last entry of `iterations`); the window is the largest of the
/// models that the turn used.
pub fn claude_usage(value: &Value) -> Option<DriveEvent> {
    let usage = value.get("usage")?;
    let sent = |part: &Value| {
        number(&part["input_tokens"])
            + number(&part["cache_read_input_tokens"])
            + number(&part["cache_creation_input_tokens"])
    };
    let context = usage["iterations"]
        .as_array()
        .and_then(|all| all.last())
        .map(|last| sent(last) + number(&last["output_tokens"]));
    let window = value["modelUsage"].as_object().and_then(|models| {
        models
            .values()
            .filter_map(|model| model["contextWindow"].as_u64())
            .max()
    });
    Some(DriveEvent::Usage {
        usage: Usage {
            context,
            window,
            input: sent(usage),
            output: number(&usage["output_tokens"]),
            cost_usd: value["total_cost_usd"].as_f64(),
        },
    })
}

fn list(value: &Value, most: usize) -> impl Iterator<Item = &Value> {
    value
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .take(most)
}

fn number(value: &Value) -> u64 {
    value.as_u64().unwrap_or(0)
}
