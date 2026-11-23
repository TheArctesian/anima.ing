//! What a program says of a thread beside its words: the diff and the plan of a Codex turn
//! (TH101, TH102), the use of tokens (TH103) and the models that a program offers (TH104).
//! Pure: each function reads one JSON value. None of these facts names the account (THR-051).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DriveEvent, text_of};
use crate::{MAX_TEXT, held};

/// The most steps of one plan and the most models of one list that a decoder keeps.
const MAX_STEPS: usize = 100;
const MAX_MODELS: usize = 200;
/// The most bytes of one step, one model value or one model name.
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

/// A model that a program offers, for the picker of a start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChoice {
    /// The value that a start names (`--model` of Claude Code, `model` of Codex).
    pub value: String,
    /// The name for the person.
    pub name: String,
    /// The efforts of reasoning that the model takes; empty when it takes none.
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
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

/// The answer to `model/list` of Codex, or `None` when `result` is not one. A hidden model is
/// left out, as the picker of Codex leaves it out.
#[must_use]
pub fn codex_models(result: &Value) -> Option<DriveEvent> {
    let data = result.get("data")?.as_array()?;
    if !data.iter().all(|model| model.get("model").is_some()) {
        return None;
    }
    let models = data
        .iter()
        .filter(|model| model["hidden"].as_bool() != Some(true))
        .take(MAX_MODELS)
        .map(|model| ModelChoice {
            value: held(&text_of(&model["model"]), MAX_WORDS),
            name: held(&text_of(&model["displayName"]), MAX_WORDS),
            efforts: list(&model["supportedReasoningEfforts"], MAX_STEPS)
                .filter_map(|effort| effort["reasoningEffort"].as_str())
                .map(|effort| held(effort, MAX_WORDS))
                .collect(),
            default_effort: model["defaultReasoningEffort"]
                .as_str()
                .map(|effort| held(effort, MAX_WORDS)),
        })
        .collect();
    Some(DriveEvent::Models { models })
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

/// The list `models` of the answer of Claude Code to `initialize`, or `None` when the answer has
/// none. Only this list is read: the rest of the answer names the account (THR-051).
#[must_use]
pub fn claude_models(answer: &Value) -> Option<DriveEvent> {
    let models = answer.get("models")?.as_array()?;
    let models = models
        .iter()
        .take(MAX_MODELS)
        .filter(|model| model["value"].is_string())
        .map(|model| {
            let efforts: Vec<String> = if model["supportsEffort"].as_bool() == Some(true) {
                list(&model["supportedEffortLevels"], MAX_STEPS)
                    .filter_map(Value::as_str)
                    .map(|effort| held(effort, MAX_WORDS))
                    .collect()
            } else {
                Vec::new()
            };
            ModelChoice {
                value: held(&text_of(&model["value"]), MAX_WORDS),
                name: held(&text_of(&model["displayName"]), MAX_WORDS),
                efforts,
                default_effort: None,
            }
        })
        .collect();
    Some(DriveEvent::Models { models })
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
