//! The secret goes to `credentials` sealed; the audit entry and the event name only its kind (DEV-012, SEC-003, SEC-005, LOG-005, LOG-007).
//! `credentials` takes its per-device lock before this domain locks the device row.
//! A set binds the secret to the host it reads first; a host changed in between gets it refused, never sent to the new one (SEC-011).

use async_graphql::{Context, Error as GraphQlError, ID, Result};

use super::{
    credential_input::{SshCredential, checked_secret},
    error::to_graphql,
    input::checked,
    ssh::DeviceSsh,
    trail,
};
use crate::{
    api_errors::{begin, commit, internal, invalid, schema_dependency},
    fleet::{
        credentials::{Vault, clear_ssh, operational, store_ssh},
        devices::{Device, Error, FieldError, Missing, hub_device, store},
        events::Trail,
    },
    ids::DeviceId,
};

pub(super) fn device_arg(id: &ID) -> Result<DeviceId, FieldError> {
    id.parse()
        .map_err(|_| FieldError::new("deviceId", "That device id is not valid."))
}

/// `UnknownDevice` is the operator's `NOT_FOUND`; any other failure is `INTERNAL` with its
/// cause kept as the hidden source. No `credentials` error message holds a secret.
pub(super) fn credential_error(
    error: crate::fleet::credentials::Error,
    device: DeviceId,
) -> GraphQlError {
    match error {
        crate::fleet::credentials::Error::UnknownDevice => {
            to_graphql(Error::NotFound(Missing::Device(device)))
        }
        other => internal(other),
    }
}

fn settings(device: Device, credential_stored: bool) -> DeviceSsh {
    DeviceSsh {
        device: device.id,
        settings: device.ssh,
        credential_stored: Some(credential_stored),
    }
}

/// Replaces the device's SSH secret, then audits and records the change.
pub(super) async fn set(
    ctx: &Context<'_>,
    device: &ID,
    credential: SshCredential,
) -> Result<DeviceSsh> {
    let SshCredential(secret) = credential;
    let id = device_arg(device);
    let kind = checked(checked_secret(&secret), id.as_ref().err().cloned())?;
    let id = id.map_err(|error| invalid(&[error]))?;
    let vault = schema_dependency::<Vault>(ctx)?;
    let trail = Trail::from_context(ctx)?;
    let mut tx = begin(ctx).await?;
    let device = store::fetch(&mut *tx, id).await.map_err(to_graphql)?;
    hub_device::check_secret(&device).map_err(to_graphql)?;
    store_ssh(&mut tx, vault, (id, &device.host), secret)
        .await
        .map_err(|error| credential_error(error, id))?;
    trail
        .write(&mut tx, trail::credential_set(&device, kind))
        .await
        .map_err(internal)?;
    commit(tx).await?;
    Ok(settings(device, true))
}

/// Removes the device's SSH secret. Only a removal is audited and recorded: clearing a device
/// that has no secret changes nothing.
pub(super) async fn clear(ctx: &Context<'_>, device: &ID) -> Result<DeviceSsh> {
    let id = device_arg(device).map_err(|error| invalid(&[error]))?;
    let trail = Trail::from_context(ctx)?;
    let mut tx = begin(ctx).await?;
    let removed = clear_ssh(&mut tx, id)
        .await
        .map_err(|error| credential_error(error, id))?;
    let device = store::fetch(&mut *tx, id).await.map_err(to_graphql)?;
    if removed {
        trail
            .write(&mut tx, trail::credential_cleared(&device))
            .await
            .map_err(internal)?;
    }
    commit(tx).await?;
    Ok(settings(device, false))
}

/// Ends the grants of operational keys of a device that is to be deleted, in `tx`, and writes
/// one audit entry and one event for each key that its server had: the hub cannot take a
/// fetched value back, so it says which keys to revoke at their provider (K-E5 Section 2.1).
pub(super) async fn end_key_grants(
    ctx: &Context<'_>,
    tx: &mut sqlx::PgConnection,
    device: DeviceId,
) -> Result<()> {
    let had = operational::forget_device(tx, device)
        .await
        .map_err(internal)?;
    for entry in had {
        Trail::from_context(ctx)?
            .write(tx, entry)
            .await
            .map_err(internal)?;
    }
    Ok(())
}
