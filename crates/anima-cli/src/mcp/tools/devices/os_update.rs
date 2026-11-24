use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    devices::{
        get::resolve,
        os_update::{
            DeviceOsUpdate, RequestDeviceOsUpdate, device_os_update, request_device_os_update,
        },
    },
    hub::Hub,
    mcp::{
        arguments::parse,
        catalogue::Access,
        definition::{Definition, Effect},
        outcome::ToolError,
    },
};

#[derive(Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Action {
    #[default]
    Status,
    Check,
    Download,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    device: String,
    #[serde(default)]
    action: Action,
}

pub fn definition() -> Definition {
    Definition::new::<Input>(
        "anima_device_os_update",
        Access::Login,
        Effect::Sets,
        "Read Anima OS update status, check the signed release feed or stage an update. Requires private SSH with an accepted host key and saved credentials. Downloads continue on the device after disconnecting. Activation and rollback restart the device and must be confirmed in the desktop app or CLI.",
    ).returns(&[("update", "object", "Current update state or a requested acknowledgement; a request is not completion.")])
}

pub async fn call(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: Input = parse(arguments)?;
    let device_id = resolve(hub, &input.device).await?;
    let action = match input.action {
        Action::Status => None,
        Action::Check => Some(request_device_os_update::OsUpdateAction::CHECK),
        Action::Download => Some(request_device_os_update::OsUpdateAction::DOWNLOAD),
    };
    let update = if let Some(action) = action {
        let answer = hub
            .run::<RequestDeviceOsUpdate>(request_device_os_update::Variables {
                device_id,
                action,
                confirmed: false,
            })
            .await?;
        json!({"requested":answer.request_device_os_update})
    } else {
        hub.run::<DeviceOsUpdate>(device_os_update::Variables { device_id })
            .await?
            .device_os_update
    };
    Ok(json!({"update":update}))
}
