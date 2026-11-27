//! Read Compose apply metadata. File submission belongs to the signed-in operator.
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    containers::apply,
    hub::Hub,
    mcp::{
        arguments::parse,
        catalogue::Access,
        definition::{Definition, Effect},
        outcome::ToolError,
    },
};

use super::DeviceTool;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Status {
    /// The apply ID returned to the operator.
    id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Targets {}

pub(super) fn definition_of(tool: DeviceTool) -> Definition {
    if tool == DeviceTool::ApplicationApplyStatus {
        Definition::new::<Status>(
            "anima_application_apply_status",
            Access::Read,
            Effect::Reads,
            "Read the state and health of a managed Compose apply by its ID. A failed attempt with reason secret_failed needs a fresh operator submission.",
        ).returns(&[("apply", "object", "The apply ID, application ID, device ID, state, health and reason.")])
    } else {
        Definition::new::<Targets>(
            "anima_application_apply_targets",
            Access::Read,
            Effect::Reads,
            "List devices that recently advertised a paired Compose applications service.",
        )
        .returns(&[("devices", "array", "One ID and name per device.")])
    }
}

pub(super) async fn call(
    tool: DeviceTool,
    hub: &Hub,
    arguments: Option<JsonObject>,
) -> Result<Value, ToolError> {
    if tool == DeviceTool::ApplicationApplyStatus {
        let requested: Status = parse(arguments)?;
        Ok(json!({ "apply": apply::status(hub, requested.id).await? }))
    } else {
        let _: Targets = parse(arguments)?;
        Ok(json!({ "devices": apply::list_targets(hub).await? }))
    }
}
