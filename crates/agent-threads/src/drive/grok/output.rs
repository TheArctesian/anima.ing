use crate::drive::{DriveEvent, MAX_LINE, PlanStep, ProgramRequestId, StepState};
use crate::{MAX_OUTPUT, MAX_TEXT, held};
use serde_json::Value;

#[must_use]
pub fn read(line: &[u8]) -> Vec<DriveEvent> {
    if line.len() > MAX_LINE {
        return vec![unknown("Grok wrote a line over the limit", true)];
    }
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return vec![unknown("Grok wrote invalid JSON", true)];
    };
    if value["jsonrpc"] != "2.0" {
        return vec![unknown("Grok wrote an invalid ACP frame", true)];
    }
    match value["method"].as_str() {
        Some("session/request_permission") => vec![super::permission::request(&value).map_or_else(
            || unknown("Grok wrote an invalid permission request", true),
            DriveEvent::Request,
        )],
        Some(_) if value.get("id").is_some() => {
            vec![unknown("Grok requested an unsupported client method", true)]
        }
        Some("session/update") => update(&value["params"]["update"]),
        Some(_) => Vec::new(),
        None => ProgramRequestId::from_json(&value["id"]).map_or_else(
            || vec![unknown("Grok wrote a reply without an ID", true)],
            |id| {
                vec![DriveEvent::Reply {
                    id,
                    ok: value.get("error").is_none() && value.get("result").is_some(),
                    result: value.get("result").cloned().unwrap_or(Value::Null),
                }]
            },
        ),
    }
}

fn update(update: &Value) -> Vec<DriveEvent> {
    let kind = update["sessionUpdate"].as_str().unwrap_or_default();
    let text = || {
        held(
            update["content"]["text"].as_str().unwrap_or_default(),
            MAX_TEXT,
        )
    };
    match kind {
        "agent_message_chunk" => vec![DriveEvent::TextDelta { text: text() }],
        "agent_thought_chunk" => vec![DriveEvent::ReasoningDelta { text: text() }],
        "tool_call" => vec![DriveEvent::ToolCall {
            id: held(update["toolCallId"].as_str().unwrap_or_default(), 1024),
            name: held(update["title"].as_str().unwrap_or("Grok tool"), 1024),
            input: held(&update["rawInput"].to_string(), MAX_OUTPUT),
            parent: None,
        }],
        "tool_call_update" => tool_update(update),
        "plan" => vec![DriveEvent::Plan {
            explanation: None,
            steps: update["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .take(200)
                .map(|entry| PlanStep {
                    step: held(entry["content"].as_str().unwrap_or_default(), MAX_TEXT),
                    state: match entry["status"].as_str() {
                        Some("completed") => StepState::Completed,
                        Some("in_progress") => StepState::InProgress,
                        _ => StepState::Pending,
                    },
                })
                .collect(),
        }],
        _ => Vec::new(),
    }
}

fn tool_update(update: &Value) -> Vec<DriveEvent> {
    if !matches!(update["status"].as_str(), Some("completed" | "failed")) {
        return Vec::new();
    }
    let output = update
        .get("rawOutput")
        .map_or_else(|| update["content"].to_string(), Value::to_string);
    vec![DriveEvent::ToolOutput {
        id: held(update["toolCallId"].as_str().unwrap_or_default(), 1024),
        output: held(&output, MAX_OUTPUT),
        failed: update["status"] == "failed",
        decided_by: None,
    }]
}

fn unknown(what: &str, stops_turn: bool) -> DriveEvent {
    DriveEvent::Unknown {
        what: what.to_owned(),
        stops_turn,
    }
}
