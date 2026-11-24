//! THR-172: negotiate only the one additive mobile capability field, before reducing errors.

use graphql_client::QueryBody;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{Hub, HubError, answer, encode};

const LEGACY: &str = include_str!("../capabilities/legacy.graphql");

impl Hub {
    pub(crate) async fn send_capabilities<V: Serialize, R: DeserializeOwned>(
        &self,
        body: &QueryBody<V>,
    ) -> Result<R, HubError> {
        let (status, bytes) = self.post(encode(body)).await?;
        if legacy_required(status, &bytes) {
            return self
                .send(&QueryBody {
                    variables: &body.variables,
                    query: LEGACY,
                    operation_name: body.operation_name,
                })
                .await;
        }
        answer::read(status, &bytes)
    }
}

fn legacy_required(status: u16, bytes: &[u8]) -> bool {
    if !matches!(status, 200 | 400) {
        return false;
    }
    let Ok(Value::Object(envelope)) = serde_json::from_slice::<Value>(bytes) else {
        return false;
    };
    if envelope
        .keys()
        .any(|key| !matches!(key.as_str(), "data" | "errors" | "extensions"))
    {
        return false;
    }
    if envelope
        .get("extensions")
        .is_some_and(|value| !value.is_object() || value.get("code").is_some())
    {
        return false;
    }
    if envelope.get("data").is_some_and(|value| !value.is_null()) {
        return false;
    }
    let Some(Value::Array(errors)) = envelope.get("errors") else {
        return false;
    };
    !errors.is_empty() && errors.iter().all(unknown_mobile_field)
}

fn unknown_mobile_field(error: &Value) -> bool {
    let Some(error) = error.as_object() else {
        return false;
    };
    if error.get("path").is_some_and(|value| !value.is_null()) {
        return false;
    }
    if !locations_fit(error.get("locations")) {
        return false;
    }
    match error.get("extensions") {
        None | Some(Value::Null) => {}
        Some(Value::Object(extensions)) if !extensions.contains_key("code") => {}
        _ => return false,
    }
    matches!(
        error.get("message").and_then(Value::as_str),
        Some(
            "Unknown field \"mobileTools\" on type \"DeviceCapabilities\"."
                | "Cannot query field \"mobileTools\" on type \"DeviceCapabilities\"."
        )
    )
}

fn locations_fit(value: Option<&Value>) -> bool {
    #[derive(serde::Deserialize)]
    struct Location {
        line: u32,
        column: u32,
    }
    value.is_none_or(|value| {
        serde_json::from_value::<Vec<Location>>(value.clone()).is_ok_and(|locations| {
            !locations.is_empty()
                && locations
                    .iter()
                    .all(|location| location.line > 0 && location.column > 0)
        })
    })
}
