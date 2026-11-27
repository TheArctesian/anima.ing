//! Shared transcript projection and redaction for local and managed runner uploads.
use agent_threads::{control::Change, drive::DriveEvent};
use daemon_core::wire::MAX_THREAD_ENTRY;
use serde_json::{Value, json};

pub(crate) fn content(change: Change) -> Option<Value> {
    let value = match change {
        Change::State { state, .. } => json!({"change": "state", "state": state}),
        Change::Said {
            event:
                DriveEvent::Account { .. }
                | DriveEvent::Reply { .. }
                | DriveEvent::Models { .. }
                | DriveEvent::Commands { .. },
        } => return None,
        other => serde_json::to_value(other).ok()?,
    };
    let mut value = value;
    redact(&mut value);
    if serde_json::to_vec(&value).ok()?.len() > MAX_THREAD_ENTRY {
        return Some(json!({"change": "omitted", "reason": "entry_size"}));
    }
    Some(value)
}

fn redact(value: &mut Value) {
    match value {
        Value::String(text) => {
            text.retain(|ch| ch != '\0');
            *text = redaction::redact_document(text);
        }
        Value::Array(rows) => rows.iter_mut().for_each(redact),
        Value::Object(fields) => {
            fields.retain(|key, _| !key.contains('\0'));
            fields.values_mut().for_each(redact);
        }
        _ => (),
    }
}
