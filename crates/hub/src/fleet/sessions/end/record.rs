//! A session's end is written once, in one transaction (K4 Section 5, D-9, LOG-005).
//! Nothing here holds session content, the connection ID, a token or the far side's text
//! (RA-008, LOG-007).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::runtime::Handle;

use super::{
    cause::{Cause, End, TASK_STOPPED},
    end_row,
};
use crate::{
    clock::Clock,
    fleet::{
        events::{self, Bus, NewEvent, Source},
        sessions::{Actor, EndReason, Error, PendingTicket, Redeemed, Session, SessionEnd},
    },
    ids::SessionId,
};

#[derive(Clone)]
pub(in crate::fleet::sessions) struct Records {
    pub(in crate::fleet::sessions) db: PgPool,
    pub(in crate::fleet::sessions) bus: Bus,
    pub(in crate::fleet::sessions) clock: Arc<dyn Clock>,
}

impl Records {
    pub(in crate::fleet::sessions) fn new(db: &PgPool, bus: &Bus, clock: &Arc<dyn Clock>) -> Self {
        Self {
            db: db.clone(),
            bus: bus.clone(),
            clock: Arc::clone(clock),
        }
    }
}

/// Dropped before [`Unfinished::finish`] (a panic), it writes the end from a new task (MV-003).
/// Without a running runtime the row stays live until the hub's next start ends it (`close_stale`).
pub(in crate::fleet::sessions) struct Unfinished {
    records: Records,
    redeemed: Option<Redeemed>,
}

impl Unfinished {
    pub(in crate::fleet::sessions) const fn new(records: Records, redeemed: Redeemed) -> Self {
        Self {
            records,
            redeemed: Some(redeemed),
        }
    }

    pub(in crate::fleet::sessions) fn redeemed(&mut self) -> &mut Redeemed {
        self.redeemed
            .as_mut()
            .expect("only `finish` takes the session, and it consumes the guard")
    }

    pub(in crate::fleet::sessions) async fn finish(mut self, end: &End) {
        if let Some(redeemed) = self.redeemed.take() {
            finish(&self.records, redeemed, end).await;
        }
    }
}

impl Drop for Unfinished {
    fn drop(&mut self) {
        let Some(redeemed) = self.redeemed.take() else {
            return;
        };
        let session = redeemed.ticket.session;
        tracing::error!(%session, "the tunnel task stopped before the session ended");
        let records = self.records.clone();
        let end = redeemed
            .tunnel
            .signalled()
            .map_or_else(|| End::worded(Cause::GatewayError, TASK_STOPPED), End::from);
        if let Ok(runtime) = Handle::try_current() {
            runtime.spawn(async move { finish(&records, redeemed, &end).await });
        }
    }
}

/// The tunnel registration drops after the ended row is published, so `endSession` reads the
/// ended row once it sees the tunnel gone.
pub(in crate::fleet::sessions) async fn finish(records: &Records, redeemed: Redeemed, end: &End) {
    let Redeemed { ticket, tunnel } = redeemed;
    let reason = end.cause.ending().reason;
    let written = match write(records, &ticket, end).await {
        Err(_) => write(records, &ticket, end).await,
        first => first,
    };
    match written {
        Ok(Some(ended)) => {
            log_ended(ticket.session, reason);
            tunnel.live().publish_change(ended);
        }
        Ok(None) => log_ended_before(ticket.session, reason),
        Err(error) => log_unwritten(ticket.session, reason, &error),
    }
    drop(tunnel);
}

/// The process log lines for an end carry the session ID and reason only (LOG-007).
fn log_ended(session: SessionId, reason: EndReason) {
    tracing::info!(%session, reason = reason.as_str(), "session ended");
}

fn log_ended_before(session: SessionId, reason: EndReason) {
    tracing::info!(
        %session,
        reason = reason.as_str(),
        "the tunnel closed a session that had already ended"
    );
}

fn log_unwritten(session: SessionId, reason: EndReason, error: &Error) {
    tracing::error!(
        %session,
        reason = reason.as_str(),
        %error,
        "the end of a session could not be written"
    );
}

/// A hub-decided end is recorded as Hub and keeps the operator who started the session
/// (LOG-014 rule 1).
fn ended_by(end: &End, ticket: &PendingTicket, now: DateTime<Utc>) -> Actor {
    if let Some(by) = &end.by {
        return by.clone();
    }
    let operator = ticket.operator.clone();
    Actor {
        operator: if end.cause.decided_by_hub() {
            operator.by_hub()
        } else {
            operator
        },
        client: ticket.client,
        at: now,
    }
}

async fn write(
    records: &Records,
    ticket: &PendingTicket,
    end: &End,
) -> Result<Option<Session>, Error> {
    let ending = end.cause.ending();
    let now = records.clock.now();
    let by = ended_by(end, ticket, now);
    let ended = SessionEnd {
        at: now,
        reason: ending.reason,
    };
    let mut tx = records.db.begin().await?;
    let Some(row) = end_row(&mut tx, ticket.session, ended, &by).await? else {
        return Ok(None);
    };
    let event = NewEvent {
        severity: ending.severity,
        source: Source::Session,
        device: row.device.map(|device| (device, row.device_name.clone())),
        message: end.message.of(ticket.protocol).to_owned(),
        actor: by.operator.actor.kind(),
    };
    events::record(&mut tx, &records.bus, now, event).await?;
    tx.commit().await?;
    Ok(Some(row))
}
