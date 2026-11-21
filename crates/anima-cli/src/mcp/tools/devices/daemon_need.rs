//! `anima_daemon_set_need`: says what a device's daemon waits for from a person at the device,
//! or clears it (R-NEW-daemonneed). It takes no secret and makes no trust decision: it only
//! changes what the device page says. The device name comes from the hub; the server cleans and
//! caps each string (MCP-008).

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::daemons::documents::{DaemonNeedSet, daemon_need_set};
use crate::devices::get::resolve;
use crate::hub::Hub;
use crate::mcp::arguments::{json, object, parse};
use crate::mcp::catalogue::Access;
use crate::mcp::definition::{Definition, Effect};
use crate::mcp::outcome::ToolError;

/// What the daemon waits for (K1 `DaemonNeed`).
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NeedInput {
    /// macOS waits for the person to allow the daemon in the Keychain.
    Keychain,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetInput {
    /// The device's ID or its exact name.
    device: String,
    /// What the daemon waits for; null or left out clears it.
    need: Option<NeedInput>,
}

pub fn definition() -> Definition {
    Definition::new::<SetInput>(
        "anima_daemon_set_need",
        Access::Login,
        Effect::Sets,
        "Says what the daemon of one device waits for from a person at that device (KEYCHAIN: \
         macOS asks the person to allow the daemon in the Keychain), or clears it when need is \
         null. The device page then tells the operator. The desktop app of that computer does \
         this by itself, and the next metrics batch of the daemon clears it; use it only to \
         correct a wrong state. Returns the device.",
    )
    .returns(&[(
        "device",
        "object",
        "id, name, daemonNeed (KEYCHAIN or null).",
    )])
}

pub async fn set(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: SetInput = parse(arguments)?;
    let device_id = resolve(hub, &input.device).await?;
    let need = input.need.map(|need| match need {
        NeedInput::Keychain => daemon_need_set::DaemonNeed::KEYCHAIN,
    });
    let device = hub
        .run::<DaemonNeedSet>(daemon_need_set::Variables { device_id, need })
        .await?
        .set_daemon_need;
    Ok(Value::Object(object([("device", json(&device))])))
}
