//! A changed switch's audit entry and event, written in the mutation's own transaction (LOG-004,
//! LOG-005, LOG-007).

use serde_json::json;

use crate::{
    engine::extensions::Key,
    fleet::events::{AuditAction, AuditTarget, Severity, Source, TrailEntry},
};

pub(super) fn switched(key: Key, on: bool) -> TrailEntry {
    let word = if on { "on" } else { "off" };
    TrailEntry {
        source: Source::Hub,
        severity: Severity::Info,
        device: None,
        message: format!("Extension {} set {word}.", key.name()),
        action: if on {
            AuditAction::Activate
        } else {
            AuditAction::Deactivate
        },
        target: AuditTarget::Setting(key.entry().setting),
        change: json!({ "key": key.as_str(), "on": on }),
    }
}
