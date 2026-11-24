//! The final reports of a model item (K10 Section 7, with `model` of K11): the final state and
//! its outcome word, its cause and its detail.

use daemon_core::model_work::cause::Cause;
use daemon_core::wire::WorkState;
use serde_json::{Value, json};
use uuid::Uuid;

/// The final report of attempt `attempt` of item `id` after its start (K10 Section 7, with
/// `model`): `done` with `succeeded`, else `failed` with its cause, also a cause that would
/// refuse the item before its start.
#[must_use]
pub fn ended(id: Uuid, attempt: u32, outcome: Result<Option<Value>, Cause>, seconds: u64) -> Value {
    match outcome {
        Ok(model) => {
            let mut body = closed((id, attempt), WorkState::Done, None, seconds);
            if let Some(model) = model {
                body["result"]["model"] = model;
            }
            body
        }
        Err(cause) => closed((id, attempt), WorkState::Failed, Some(cause), seconds),
    }
}

/// The refusal of attempt `attempt` of item `id` before its start: the hub takes `refused` only
/// from `claimed` (K10 Section 7).
#[must_use]
pub fn refused(id: Uuid, attempt: u32, cause: Cause) -> Value {
    closed((id, attempt), WorkState::Refused, Some(cause), 0)
}

/// A final report of `(id, attempt)`, whose outcome is the word of its final `state`.
fn closed(item: (Uuid, u32), state: WorkState, cause: Option<Cause>, seconds: u64) -> Value {
    let outcome = match state {
        WorkState::Done => "succeeded",
        WorkState::Refused => "refused",
        WorkState::Stopped => "stopped",
        _ => "failed",
    };
    let mut result = json!({
        "outcome": outcome, "cause": cause.map(Cause::as_str), "seconds": seconds,
        "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" },
    });
    if let Some(detail) = cause.and_then(Cause::detail) {
        result["detail"] = json!(detail);
    }
    json!({ "id": item.0, "attempt": item.1, "state": state.as_str(), "result": result })
}

/// The report of an item that ended at a stop of the hub.
#[must_use]
pub fn stopped(id: Uuid, attempt: u32, seconds: u64) -> Value {
    closed((id, attempt), WorkState::Stopped, None, seconds)
}
