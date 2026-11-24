//! Refuses a start whose stored credential was entered for another host (SEC-011); the tunnels check again on open.
//! The credential is opened only to read its bound host and is dropped at once.
//! No log or message here holds a host or a secret (LOG-007).

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{
    fleet::{
        credentials::{self, Vault},
        devices::Device,
        events::{self, ActorKind, Bus, NewEvent, Severity, Source},
        sessions::{Conflict, Desktop, Error, Protocol, find_device},
    },
    ids::DeviceId,
};

/// The request caused the refusal, so the event records `by`, the request's kind (LOG-013).
pub(in crate::fleet::sessions) async fn refuse_moved(
    (db, vault, bus): (&PgPool, &Vault, &Bus),
    (device, protocol): (DeviceId, Protocol),
    by: ActorKind,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let device = find_device(db, device).await?;
    let opened = match Desktop::of(protocol) {
        Some(desktop) => {
            let bound = (device.id, device.host.as_str());
            credentials::load_password(db, vault, bound, desktop.password())
                .await
                .map(drop)
        }
        None => credentials::load_ssh(db, vault, (device.id, &device.host))
            .await
            .map(drop),
    };
    match opened {
        Ok(()) => Ok(()),
        Err(credentials::Error::EnteredForOtherHost) => {
            let conflict = Conflict::CredentialMoved(protocol);
            record(db, bus, now, event(&device, conflict, by)).await?;
            Err(Error::Conflict(conflict))
        }
        Err(error) => Err(Error::Credentials(error)),
    }
}

fn event(device: &Device, conflict: Conflict, actor: ActorKind) -> NewEvent {
    NewEvent {
        severity: Severity::Warning,
        source: Source::Session,
        device: Some((device.id, device.name.clone())),
        message: conflict.to_string(),
        actor,
    }
}

async fn record(db: &PgPool, bus: &Bus, now: DateTime<Utc>, event: NewEvent) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    events::record(&mut tx, bus, now, event).await?;
    tx.commit().await?;
    Ok(())
}
