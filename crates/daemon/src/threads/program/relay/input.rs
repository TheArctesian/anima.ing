//! The Codex address enters only the holder's private child-input queue.

use serde_json::{Value, json};

use super::{Lease, upstream};
use crate::threads::wire::Refusal;

impl Lease {
    /// One JSON-RPC frame only: embedded line ends must not smuggle an unrouted start.
    pub fn input(&self, line: String) -> Result<String, Refusal> {
        if self.program != upstream::Program::Codex {
            return Ok(line);
        }
        let mut frame: Value = serde_json::from_str(&line).map_err(|_| Refusal::BadInput)?;
        let frame = frame.as_object_mut().ok_or(Refusal::BadInput)?;
        if matches!(
            frame.get("method").and_then(Value::as_str),
            Some("thread/start" | "thread/resume" | "thread/fork")
        ) {
            let params = frame.entry("params").or_insert_with(|| json!({}));
            let params = params.as_object_mut().ok_or(Refusal::BadInput)?;
            params.insert("modelProvider".to_owned(), json!("anima_relay"));
            let config = params.entry("config").or_insert_with(|| json!({}));
            if config.is_null() {
                *config = json!({});
            }
            let config = config.as_object_mut().ok_or(Refusal::BadInput)?;
            config.retain(|name, _| !name.starts_with("model_providers.anima_relay."));
            if let Some(providers) = config
                .get_mut("model_providers")
                .and_then(Value::as_object_mut)
            {
                providers.remove("anima_relay");
            }
            config.insert("model_provider".to_owned(), json!("anima_relay"));
            config.insert(
                "model_providers.anima_relay".to_owned(),
                json!({
                    "name": "Anima relay",
                    "base_url": self.address,
                    "wire_api": "responses",
                    "requires_openai_auth": true,
                    "supports_websockets": true,
                    "http_headers": {"x-anima-relay": self.secret},
                }),
            );
        }
        let mut line = serde_json::to_string(frame).map_err(|_| Refusal::BadInput)?;
        line.push('\n');
        Ok(line)
    }
}
