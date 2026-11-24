use crate::{
    drive::{DriveEvent, Effort, EffortWords, Model, ModelChoice},
    held,
};
use serde_json::Value;

/// Reads only model metadata; account and authentication fields never leave the adapter.
#[must_use]
pub fn models(state: &Value) -> Option<DriveEvent> {
    let rows = state.get("availableModels")?.as_array()?;
    let models = rows
        .iter()
        .take(200)
        .filter_map(|row| {
            let value = row["modelId"].as_str()?;
            Model::new(value).ok()?;
            let meta = &row["_meta"];
            let efforts: Vec<String> = meta["reasoningEfforts"]
                .as_array()
                .into_iter()
                .flatten()
                .take(32)
                .filter_map(|effort| effort["id"].as_str())
                .filter(|effort| serde_json::from_value::<Effort>(Value::from(*effort)).is_ok())
                .map(str::to_owned)
                .collect();
            let default_effort = meta["reasoningEffort"]
                .as_str()
                .filter(|effort| efforts.iter().any(|known| known == effort))
                .map(str::to_owned);
            let effort_words = meta["reasoningEfforts"]
                .as_array()
                .into_iter()
                .flatten()
                .take(32)
                .filter_map(|effort| {
                    Some(EffortWords {
                        effort: held(effort["id"].as_str()?, 32),
                        description: held(effort["description"].as_str()?, 1024),
                    })
                })
                .collect();
            Some(ModelChoice {
                value: value.to_owned(),
                name: held(row["name"].as_str().unwrap_or(value), 1024),
                efforts,
                default_effort,
                effort_words,
                default: state["currentModelId"].as_str() == Some(value),
                ..ModelChoice::default()
            })
        })
        .collect();
    Some(DriveEvent::Models { models })
}
