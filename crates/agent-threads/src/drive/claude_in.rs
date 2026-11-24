//! The lines that the runner writes to the standard input of `claude -p --input-format
//! stream-json` (TH10 Section 2b.1, TH18 Section 2.3). Each is one line of JSON with no line end.

use serde_json::{Value, json};

use super::DriveError;
use super::launch::{Mode, Model};
use super::prompt::{Attached, Prompt};
use super::request::{Answer, ProgramRequestId, Request};

/// The writer of the input lines of Claude Code.
pub struct ClaudeInput;

impl ClaudeInput {
    /// A prompt of the person. A prompt sent during a turn is queued by the program (TH10 2.2).
    #[must_use]
    pub fn prompt(text: &str) -> String {
        json!({"type": "user", "message": {"role": "user", "content": text}}).to_string()
    }

    /// A prompt with its files (TH111): an image or a PDF with bytes inline goes as a block
    /// (measured on 2.1.311, 2026-11-21); each other file goes as its path in the words.
    #[must_use]
    pub fn prompt_with(prompt: &Prompt) -> String {
        if prompt.attachments.is_empty() {
            return Self::prompt(&prompt.text);
        }
        let words = prompt.words(|file| file.kind.inline().is_none());
        let mut content = vec![json!({"type": "text", "text": words})];
        for file in &prompt.attachments {
            content.extend(block(&file.kind));
        }
        json!({"type": "user", "message": {"role": "user", "content": content}}).to_string()
    }

    /// The first line of a thread: its answer names the models of the program (TH10 2b.1,
    /// TH104). No hooks are given, so the program keeps its own.
    #[must_use]
    pub fn initialize(id: &str) -> String {
        json!({"type": "control_request", "request_id": id, "request": {"subtype": "initialize", "hooks": null}})
            .to_string()
    }

    /// Stops the turn and keeps the process (TH10 2b.1).
    #[must_use]
    pub fn interrupt(id: &str) -> String {
        json!({"type": "control_request", "request_id": id, "request": {"subtype": "interrupt"}})
            .to_string()
    }

    /// The model of the next turns (TH110; measured on Claude Code 2.1.311 between two turns).
    #[must_use]
    pub fn set_model(id: &str, model: &Model) -> String {
        json!({"type": "control_request", "request_id": id, "request": {"subtype": "set_model", "model": model.as_str()}})
            .to_string()
    }

    /// The permission mode of the next turns (TH110; measured as [`Self::set_model`]).
    #[must_use]
    pub fn set_permission_mode(id: &str, mode: Mode) -> String {
        json!({"type": "control_request", "request_id": id, "request": {"subtype": "set_permission_mode", "mode": mode.claude()}})
            .to_string()
    }

    /// The answer to a request. "Allow for the session" sends back one rule that the request
    /// itself offered, never a rule of the runner (THR-030).
    ///
    /// # Errors
    /// [`DriveError::AnswerDoesNotFit`] when the answer does not fit the request.
    pub fn answer(request: &Request, answer: &Answer) -> Result<String, DriveError> {
        answer.check(request)?;
        let response = match answer {
            Answer::Allow => json!({"behavior": "allow", "updatedInput": request.input}),
            Answer::AllowForSession { rule } => json!({
                "behavior": "allow",
                "updatedInput": request.input,
                "updatedPermissions": [request.offered[*rule]],
            }),
            Answer::Deny { message } => json!({"behavior": "deny", "message": message}),
            Answer::Choose { answers } => {
                let mut input = match &request.input {
                    Value::Object(_) => request.input.clone(),
                    _ => Value::Object(serde_json::Map::new()),
                };
                let chosen: serde_json::Map<String, Value> = answers
                    .iter()
                    .map(|(key, choice)| (key.clone(), Value::from(choice.as_str())))
                    .collect();
                input["answers"] = Value::Object(chosen);
                json!({"behavior": "allow", "updatedInput": input})
            }
        };
        Ok(success(&request.id, &response))
    }

    /// The refusal of a control request of a kind the runner does not know (THR-046).
    #[must_use]
    pub fn refuse(id: &ProgramRequestId, why: &str) -> String {
        json!({
            "type": "control_response",
            "response": {"subtype": "error", "request_id": id.json(), "error": why},
        })
        .to_string()
    }
}

fn success(id: &ProgramRequestId, response: &Value) -> String {
    json!({
        "type": "control_response",
        "response": {"subtype": "success", "request_id": id.json(), "response": response},
    })
    .to_string()
}

/// The content block of a file whose bytes go inline.
fn block(kind: &Attached) -> Option<Value> {
    let (block, mime, data) = match kind {
        Attached::Image {
            kind,
            inline: Some(data),
        } => ("image", kind.mime(), data),
        Attached::Pdf { inline: Some(data) } => ("document", "application/pdf", data),
        _ => return None,
    };
    Some(json!({"type": block, "source": {"type": "base64", "media_type": mime, "data": data}}))
}
