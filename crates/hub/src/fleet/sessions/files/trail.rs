//! Each transfer writes one audit entry, never content, and one event (K4 Section 9, LOG-005,
//! LOG-007). The path stays in the audit entry: the event log is read more widely.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;

use super::{
    protocol::Refusal,
    tickets::FilesTicket,
    transfer::{Direction, Outcome, Transfer},
};
use crate::{
    clock::Clock,
    engine::locations,
    fleet::{
        devices::Device,
        events::{
            self, ActorKind, AuditAction, AuditTarget, Bus, NewEvent, Severity, Source, TrailEntry,
        },
        sessions::{self, Error, Protocol},
    },
    ids::SiteId,
};

pub(super) struct Trail {
    pub(super) db: PgPool,
    pub(super) bus: Bus,
    pub(super) clock: Arc<dyn Clock>,
    pub(super) ticket: FilesTicket,
    pub(super) device_name: String,
    /// The device's site when the tunnel opened.
    pub(super) site: SiteId,
}

/// Fixed text with no path, value or device text.
pub(super) const fn sentence(direction: Direction, outcome: Outcome) -> (Severity, &'static str) {
    use Direction::{Download, Upload};
    match (direction, outcome) {
        (Download, Outcome::Done) => (
            Severity::Info,
            "An operator downloaded a file from the device.",
        ),
        (Upload, Outcome::Done) => (Severity::Info, "An operator uploaded a file to the device."),
        (Download, Outcome::Refused) => (
            Severity::Info,
            "The hub refused a file download from the device.",
        ),
        (Upload, Outcome::Refused) => (
            Severity::Info,
            "The hub refused a file upload to the device.",
        ),
        (Download, Outcome::Cancelled) => (
            Severity::Info,
            "A file download from the device was cancelled.",
        ),
        (Upload, Outcome::Cancelled) => {
            (Severity::Info, "A file upload to the device was cancelled.")
        }
        (Download, Outcome::Failed) => {
            (Severity::Warning, "A file download from the device failed.")
        }
        (Upload, Outcome::Failed) => (Severity::Warning, "A file upload to the device failed."),
    }
}

impl Trail {
    /// The audit entry, the event and its context (K-E4 Section 12) are written in one transaction.
    pub(super) async fn record(
        &self,
        transfer: &Transfer,
        outcome: Outcome,
        reason: Option<Refusal>,
    ) -> Result<(), Error> {
        let action = match transfer.direction {
            Direction::Upload => AuditAction::Upload,
            Direction::Download => AuditAction::Download,
        };
        let ticket = &self.ticket;
        let (severity, message) = sentence(transfer.direction, outcome);
        let entry = TrailEntry {
            source: Source::Session,
            severity,
            device: Some((ticket.device, self.device_name.clone())),
            message: message.to_owned(),
            action,
            target: AuditTarget::File(ticket.device),
            change: json!({
                "path": transfer.path.as_str(),
                "size": transfer.size,
                "bytes": transfer.moved,
                "outcome": outcome.as_str(),
                "reason": reason.map(Refusal::as_str),
                "sessionId": ticket.session.to_string(),
            }),
        };
        let actor = (ticket.operator.clone(), ticket.client);
        let now = self.clock.now();
        let trail = events::Trail::new(&self.bus, actor, now);
        let mut tx = self.db.begin().await?;
        let event = trail.write_event(&mut tx, entry).await?;
        let context = locations::Transfer {
            event,
            occurred_at: now,
            direction: match transfer.direction {
                Direction::Upload => locations::Direction::Upload,
                Direction::Download => locations::Direction::Download,
            },
            device: (ticket.device, self.site),
            actor: locations::Actor {
                kind: ticket.operator.actor.kind().into(),
                name: Some(ticket.operator.name.clone()),
                client_device: None,
            },
        };
        locations::record_transfer(&mut tx, context).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// In the words of the terminal's `HOST_KEY_MISMATCH` end (SEC-007).
pub(super) const HOST_KEY_CHANGED: &str = "The device presented an SSH host key that is not the \
                                           accepted one, so the hub opened no files tunnel \
                                           and sent no credential. Test the connection and \
                                           check the new key before accepting it.";

/// The request that opened the tunnel caused the attempt, so the event records `actor` (LOG-013).
pub(super) async fn host_key_changed(
    (db, bus): (&PgPool, &Bus),
    now: DateTime<Utc>,
    device: &Device,
    actor: ActorKind,
) -> Result<(), Error> {
    let event = NewEvent {
        severity: Severity::Error,
        source: Source::Session,
        device: Some((device.id, device.name.clone())),
        message: HOST_KEY_CHANGED.to_owned(),
        actor,
    };
    record(db, bus, now, event).await
}

/// The stored SSH credential was entered for another host (SEC-011). The request that opened
/// the tunnel caused the refusal, so the event records `actor` (LOG-013).
pub(super) async fn credential_moved(
    (db, bus): (&PgPool, &Bus),
    now: DateTime<Utc>,
    device: &Device,
    actor: ActorKind,
) -> Result<(), Error> {
    let event = NewEvent {
        severity: Severity::Warning,
        source: Source::Session,
        device: Some((device.id, device.name.clone())),
        message: sessions::credential_moved(Protocol::Ssh).to_owned(),
        actor,
    };
    record(db, bus, now, event).await
}

async fn record(db: &PgPool, bus: &Bus, now: DateTime<Utc>, event: NewEvent) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    events::record(&mut tx, bus, now, event).await?;
    tx.commit().await?;
    Ok(())
}
