//! Wake-on-LAN through the site's wake relay over the hub's SSH link (RA-031, VPN-006, D-4, P2-003).
//! `wake` sends whatever the device's state; `startSession` decides whether a start needs it.
//! [`wake`] writes the event with the caller's actor kind; `wakeDevice` writes the audit entry (LOG-005, LOG-013).
//! No event, audit entry or error holds a host, an address, the MAC address or the relay's output (LOG-007).

pub(super) mod graphql;
mod refusal;
mod relay;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    fleet::{
        credentials,
        devices::{self, Device},
        events::{
            self, ActorKind, AuditAction, AuditTarget, Bus, NewAudit, NewEvent, Severity, Source,
        },
        sessions::Actor,
        ssh::{LinkError, link},
        targets::AddressPolicy,
    },
    ids::DeviceId,
};

/// A Wake-on-LAN packet the relay sent (K1 `WakeResult`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WakeSent {
    pub sent_at: DateTime<Utc>,
    pub relay: DeviceId,
    pub relay_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("device {0} not found")]
    NotFound(DeviceId),
    #[error("the device has no MAC address")]
    NoMacAddress,
    #[error("the device's site has no wake relay")]
    NoRelay,
    #[error("the device is its own site's wake relay")]
    RelayIsDevice,
    #[error("the SSH link to the wake relay failed")]
    Link(#[source] LinkError),
    #[error("wakeonlan on the wake relay did not succeed")]
    NotSent,
    #[error("device store error")]
    Devices(#[source] devices::Error),
    #[error("event store error")]
    Events(#[from] events::Error),
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

/// A failed event write is logged and does not change the result.
///
/// # Errors
/// `NotFound` for an unknown device; the [`Error`] for the first step that fails.
pub async fn wake(
    (db, vault, policy, bus): (&PgPool, &credentials::Vault, &AddressPolicy, &Bus),
    device: DeviceId,
    by: ActorKind,
    now: DateTime<Utc>,
) -> Result<WakeSent, Error> {
    let device = devices::get(db, device)
        .await
        .map_err(|error| match error {
            devices::Error::NotFound(_) => Error::NotFound(device),
            other => Error::Devices(other),
        })?;
    let sent = async {
        let mac = device.mac_address.ok_or(Error::NoMacAddress)?;
        let relay = devices::wake_relay(db, device.site)
            .await
            .map_err(Error::Devices)?;
        let relay = relay::chosen(&device, relay)?;
        let mut link = link(db, vault, policy, &relay, now)
            .await
            .map_err(Error::Link)?;
        relay::send(&mut link, mac).await?;
        Ok(WakeSent {
            sent_at: now,
            relay: relay.id,
            relay_name: relay.name,
        })
    }
    .await;
    if let Err(error) = record(db, bus, event(&device, &sent, by), now).await {
        tracing::error!(device = %device.id, %error, "the event of a Wake-on-LAN could not be written");
    }
    sent
}

fn event(device: &Device, sent: &Result<WakeSent, Error>, actor: ActorKind) -> NewEvent {
    let (severity, message) = match sent {
        Ok(sent) => (
            Severity::Info,
            format!(
                "Sent a Wake-on-LAN packet through the wake relay {}.",
                sent.relay_name
            ),
        ),
        Err(error) => (Severity::Error, refusal::sentence(error).to_owned()),
    };
    NewEvent {
        severity,
        source: Source::Session,
        device: Some((device.id, device.name.clone())),
        message,
        actor,
    }
}

async fn record(db: &PgPool, bus: &Bus, event: NewEvent, now: DateTime<Utc>) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    events::record(&mut tx, bus, now, event).await?;
    tx.commit().await?;
    Ok(())
}

pub(in crate::fleet::sessions) async fn audit(
    db: &PgPool,
    by: &Actor,
    device: DeviceId,
    sent: &Result<WakeSent, Error>,
) -> Result<(), Error> {
    let change = match sent {
        Ok(sent) => json!({
            "relayId": sent.relay.to_string(),
            "relayName": sent.relay_name,
        }),
        Err(error) => json!({ "sent": false, "reason": refusal::sentence(error) }),
    };
    let entry = NewAudit {
        operator: by.operator.clone(),
        action: AuditAction::Wake,
        target: AuditTarget::Device(device),
        client: by.client,
        change,
    };
    let mut tx = db.begin().await?;
    events::audit(&mut tx, by.at, entry).await?;
    tx.commit().await?;
    Ok(())
}
