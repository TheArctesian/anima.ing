//! The models that a program offers, for the picker of a start (TH104), with what the picker
//! shows beside each one (TH117). Pure: each function reads one JSON value. None of these facts
//! names the account (THR-051).

use std::ops::Not;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{key, line, list};
use crate::drive::{DriveEvent, text_of};
use crate::held;

/// The most models of one list, and the most efforts, speeds or programs of access of a model.
const MAX_MODELS: usize = 200;
const MAX_PARTS: usize = 32;
/// The most bytes of one model value or one model name.
const MAX_WORDS: usize = 1024;

/// A model that a program offers. Each field comes from the program; a field that the program
/// does not give is empty, `false` or `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChoice {
    /// The value that a start names (`--model` of Claude Code, `model` of Codex).
    pub value: String,
    /// The name for the person.
    pub name: String,
    /// The efforts of reasoning that the model takes; empty when it takes none.
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
    /// What the program says of each effort, for a tip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effort_words: Vec<EffortWords>,
    /// The program names it as its default model.
    #[serde(default, skip_serializing_if = "Not::not")]
    pub default: bool,
    /// A newer model takes its place: the picker keeps it under "Show more".
    #[serde(default, skip_serializing_if = "Not::not")]
    pub legacy: bool,
    /// The value of the newer model, when the program names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<String>,
    /// The program has a message that announces the model.
    #[serde(default, skip_serializing_if = "Not::not")]
    pub new: bool,
    /// The speeds other than the standard one; empty when the model has only the standard one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speeds: Vec<SpeedChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_speed: Option<String>,
    /// The programs of access that the account may ask for with this model (Codex `cyber`).
    /// Only shown: no program takes the choice from a start yet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub access_programs: Vec<String>,
}

/// One effort with the words of the program on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffortWords {
    pub effort: String,
    pub description: String,
}

/// One speed of a model: a service tier of Codex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeedChoice {
    /// The value that `start` and `set` name.
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
        .map(codex_model)
        .collect();
    Some(DriveEvent::Models { models })
}

fn codex_model(model: &Value) -> ModelChoice {
    let efforts = list(&model["supportedReasoningEfforts"], MAX_PARTS);
    let upgrade = key(&model["upgrade"], b"._[]");
    ModelChoice {
        value: held(&text_of(&model["model"]), MAX_WORDS),
        name: held(&text_of(&model["displayName"]), MAX_WORDS),
        efforts: efforts
            .clone()
            .filter_map(|effort| effort["reasoningEffort"].as_str())
            .map(|effort| held(effort, MAX_WORDS))
            .collect(),
        default_effort: model["defaultReasoningEffort"]
            .as_str()
            .map(|effort| held(effort, MAX_WORDS)),
        effort_words: efforts
            .filter_map(|effort| {
                Some(EffortWords {
                    effort: key(&effort["reasoningEffort"], b"")?,
                    description: line(&effort["description"])?,
                })
            })
            .collect(),
        default: model["isDefault"].as_bool() == Some(true),
        legacy: upgrade.is_some(),
        upgrade,
        new: line(&model["availabilityNux"]["message"]).is_some(),
        speeds: list(&model["serviceTiers"], MAX_PARTS)
            .filter_map(|tier| {
                Some(SpeedChoice {
                    id: key(&tier["id"], b".")?,
                    name: line(&tier["name"])?,
                    description: line(&tier["description"]),
                })
            })
            .collect(),
        default_speed: key(&model["defaultServiceTier"], b"."),
        access_programs: list(&model["availableAccessPrograms"]["cyber"], MAX_PARTS)
            .filter_map(|program| key(program, b"."))
            .collect(),
    }
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
                list(&model["supportedEffortLevels"], MAX_PARTS)
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
                ..ModelChoice::default()
            }
        })
        .collect();
    Some(DriveEvent::Models { models })
}
