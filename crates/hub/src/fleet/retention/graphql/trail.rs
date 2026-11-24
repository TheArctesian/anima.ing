//! A retention change writes its audit entry and event in the mutation's own transaction
//! (LOG-004, LOG-005, LOG-007).

use serde_json::json;

use crate::fleet::{
    events::{AuditAction, AuditTarget, Severity, Source, TrailEntry},
    retention::Subject,
};

pub(super) fn changed(subject: Subject, days: u16, previous: i32) -> TrailEntry {
    TrailEntry {
        source: Source::Hub,
        severity: Severity::Info,
        device: None,
        message: format!(
            "Retention of {} set to {days} days (was {previous}).",
            subject.as_str()
        ),
        action: AuditAction::Update,
        target: AuditTarget::Setting(setting_name(subject)),
        change: json!({ "subject": subject.as_str(), "days": days, "previousDays": previous }),
    }
}

const fn setting_name(subject: Subject) -> &'static str {
    match subject {
        Subject::Heartbeats => "retention.heartbeats",
        Subject::Metrics => "retention.metrics",
        Subject::Events => "retention.events",
        Subject::Deliveries => "retention.deliveries",
    }
}
