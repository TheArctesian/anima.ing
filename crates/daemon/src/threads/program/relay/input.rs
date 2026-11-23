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
            let config = params.entry("config").or_insert_with(|| json!({}));
            if config.is_null() {
                *config = json!({});
            }
            let config = config.as_object_mut().ok_or(Refusal::BadInput)?;
            config.insert("openai_base_url".to_owned(), json!(self.address));
        }
        let mut line = serde_json::to_string(frame).map_err(|_| Refusal::BadInput)?;
        line.push('\n');
        Ok(line)
    }
}
