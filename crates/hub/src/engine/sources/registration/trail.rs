//! What an attempt leaves in the audit log and the event log before an account exists (K-E1 Section 7a, LOG-004).
//! An entry never holds a key, a code, a ticket, a cookie or a state (LOG-007, PRJ-041).

use serde_json::json;

use super::attempt::App;
use crate::fleet::events::{ActorKind, AuditAction, AuditTarget, Severity, Source, TrailEntry};

const TARGET: AuditTarget = AuditTarget::Setting("sources.github.connect");

fn entry(action: AuditAction, message: &str, change: serde_json::Value) -> TrailEntry {
    TrailEntry {
        source: Source::Hub,
        severity: Severity::Info,
        device: None,
        message: message.to_owned(),
        action,
        target: TARGET,
        change,
    }
}

pub fn started(label: Option<&str>, organisation: Option<&str>, actor: ActorKind) -> TrailEntry {
    entry(
        AuditAction::Start,
        "A GitHub connection was started.",
        json!({ "label": label, "organisation": organisation, "actor": actor.as_str() }),
    )
}

/// The entry is what lets the operator delete the App after a hub restart.
pub fn app_created(app: &App) -> TrailEntry {
    entry(
        AuditAction::Create,
        "A GitHub App was made for Anima.ing.",
        json!({ "appId": app.id.to_string(), "slug": app.slug, "ownerLogin": app.owner.login }),
    )
}

pub fn cancelled() -> TrailEntry {
    entry(
        AuditAction::Cancel,
        "A GitHub connection was cancelled.",
        json!({}),
    )
}
