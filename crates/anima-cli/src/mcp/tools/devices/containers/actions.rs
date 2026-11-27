use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    containers::actions::{self, Verb},
    hub::Hub,
    mcp::{
        arguments::parse,
        catalogue::Access,
        definition::{Definition, Effect},
        outcome::ToolError,
    },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Target {
    /// The device's ID or exact name.
    device: String,
    /// The container's exact name, full ID or unique ID prefix of at least 12 digits.
    container: String,
    action: Verb,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Status {
    /// The ID returned by `anima_container_action`.
    id: String,
}

pub(super) fn queue_definition() -> Definition {
    Definition::new::<Target>(
        "anima_container_action", Access::Write, Effect::Acts,
        "Queue one start, stop or restart on a specific Docker container and device. The daemon and its local hosting policy decide whether it runs. A queued request is not completion. Use anima_container_action_status with the returned action ID.",
    ).returns(&[("action", "object", "The action ID, state and refusal reason when present.")])
}

pub(super) fn status_definition() -> Definition {
    Definition::new::<Status>(
        "anima_container_action_status", Access::Read, Effect::Reads,
        "Read the state of a container action by its ID. Use this to confirm the outcome of anima_container_action without submitting another action.",
    ).returns(&[("action", "object", "The action ID, state and refusal reason when present.")])
}

pub(super) async fn queue(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let target: Target = parse(arguments)?;
    let action = actions::queue(hub, (&target.device, &target.container), target.action).await?;
    Ok(json!({"action":action}))
}

pub(super) async fn status(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let target: Status = parse(arguments)?;
    Ok(json!({"action":actions::status(hub, target.id).await?}))
}
