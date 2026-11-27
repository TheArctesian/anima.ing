//! Device inventory: sites, devices, tags and the SSH, RDP and VNC settings.

pub mod graphql;
mod hub_device;
mod loaders;
mod mac;
mod model;
mod page;
mod records;
mod remote;
mod rules;
mod store;
#[cfg(test)]
mod tests;
mod text;
mod warnings;

use std::num::NonZeroU8;

use sqlx::{PgConnection, PgExecutor, PgPool};

use crate::api_errors::field_names;
use crate::fleet::events;
use crate::ids::{DeviceId, SiteId};

pub use hub_device::{LinkComputer, ensure_hub_device, ensure_link_computer};
pub use loaders::{DevicesById, DevicesBySite, SitesById};
pub use mac::{MacAddress, MacAddressError};
pub use model::{Device, Distribution, OperatingSystem, Site, SshAuth, SshSettings};
pub use remote::{
    MetricsSource, RdpSecurity, RdpSettings, ResizeMethod, TransferPolicy, VncSettings,
};
pub use rules::FieldError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Device(DeviceId),
    Site(SiteId),
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Device(id) => write!(f, "device {id}"),
            Self::Site(id) => write!(f, "site {id}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(Missing),
    #[error("input is not valid: {}", field_names(.0))]
    Validation(Vec<FieldError>),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    /// The audit entry or the event of the hub's own device.
    #[error("the audit entry cannot be written")]
    Trail(#[source] events::Error),
}

/// The device with this id, with its tags and SSH settings.
///
/// # Errors
/// `NotFound` when there is no such device.
pub async fn get(db: &PgPool, id: DeviceId) -> Result<Device, Error> {
    store::get(db, id).await
}

/// Every device, in byte order of the name.
///
/// # Errors
/// `Database` when the query fails.
pub async fn list(db: &PgPool) -> Result<Vec<Device>, Error> {
    store::all(db).await
}

/// Every site, in name order (VPN-009).
///
/// # Errors
/// `Database` when the query fails.
pub async fn sites(db: &PgPool) -> Result<Vec<Site>, Error> {
    store::list_sites(db).await
}

/// Records the Linux distribution read from the device (DEV-016). The device keeps its
/// `updated_at`: the operator did not change it. An equal value writes nothing.
///
/// # Errors
/// `Database` when the write fails. A deleted device is not an error: nothing is written.
pub async fn record_distribution(
    db: &PgPool,
    id: DeviceId,
    distribution: &Distribution,
) -> Result<(), Error> {
    store::set_distribution(db, id, distribution).await
}

/// Records the operating system detected on the device over SSH while the stored one is Other
/// (DEV-025); returns whether it was written. An operating system the operator chose is never
/// changed, and the device keeps its `updated_at`: the operator did not change it.
///
/// # Errors
/// `Database` when the write fails. A deleted device is not an error: nothing is written.
pub async fn record_detected_os(
    db: &PgPool,
    id: DeviceId,
    os: OperatingSystem,
) -> Result<bool, Error> {
    store::set_detected_os(db, id, os).await
}

/// # Errors
/// `NotFound` when there is no such device.
pub async fn in_maintenance(db: &PgPool, id: DeviceId) -> Result<bool, Error> {
    store::in_maintenance(db, id).await
}

/// The device that sends Wake-on-LAN packets at the site, if one is set.
///
/// # Errors
/// `NotFound` when there is no such site.
pub async fn wake_relay(db: &PgPool, site: SiteId) -> Result<Option<Device>, Error> {
    store::wake_relay(db, site).await
}

/// Locks the device row (`FOR UPDATE`) until the caller's transaction ends, so no other
/// transaction changes or deletes the device meanwhile. A transaction that writes rows deleted
/// with the device takes this first, the order a device delete takes the rows in, so the two
/// cannot deadlock.
///
/// # Errors
/// `NotFound` when there is no such device.
pub async fn lock(conn: &mut PgConnection, id: DeviceId) -> Result<(), Error> {
    store::lock_device(conn, id).await
}

/// Locks the device row as [`lock`] does and returns its metrics source as it is now, so the
/// caller's transaction acts on a source no other transaction can change before it ends
/// (MET-011, MET-018).
///
/// # Errors
/// `NotFound` when there is no such device.
pub async fn lock_metrics_source(
    conn: &mut PgConnection,
    id: DeviceId,
) -> Result<MetricsSource, Error> {
    store::lock_metrics_source(conn, id).await
}

/// Sets the device's metrics source to `daemon` in the caller's transaction, which stops SSH
/// collection for it, and returns the device's name (MET-011, R-092 D-7).
///
/// # Errors
/// `NotFound` when there is no such device.
pub async fn use_daemon_metrics(conn: &mut PgConnection, id: DeviceId) -> Result<String, Error> {
    store::set_daemon_source(conn, id).await
}

/// The most live sessions the device allows, `None` for no limit (RA-036).
///
/// # Errors
/// `NotFound` when there is no such device.
pub async fn session_limit<'e>(
    db: impl PgExecutor<'e>,
    id: DeviceId,
) -> Result<Option<NonZeroU8>, Error> {
    store::remote::session_limit(db, id).await
}

/// Initializes untouched VNC settings on the authenticated private daemon device.
///
/// # Errors
/// Returns a database error when device settings cannot be read or written.
pub async fn configure_private_vnc(
    conn: &mut PgConnection,
    device: DeviceId,
    user: &str,
) -> Result<bool, Error> {
    let found = store::fetch(&mut *conn, device).await?;
    let private = found
        .host
        .parse::<std::net::Ipv6Addr>()
        .is_ok_and(|ip| ip.segments()[..3] == [0xfd41, 0x4e49, 0x4d41]);
    if private
        && found.metrics_source == MetricsSource::Daemon
        && found.kind != "hub"
        && daemon_core::inventory::is_remote_user(user)
    {
        return store::automatic::configure_vnc(conn, device, user).await;
    }
    Ok(false)
}

/// Initializes untouched connection settings for a private, enrolled device.
///
/// # Errors
/// Returns a database error if the device or its settings cannot be read or written.
pub async fn configure_private_access(
    conn: &mut PgConnection,
    device: DeviceId,
    access: &daemon_core::inventory::RemoteAccess,
) -> Result<bool, Error> {
    let found = store::fetch(&mut *conn, device).await?;
    let private = found
        .host
        .parse::<std::net::Ipv6Addr>()
        .is_ok_and(|ip| ip.segments()[..3] == [0xfd41, 0x4e49, 0x4d41]);
    if private
        && found.metrics_source == MetricsSource::Daemon
        && found.kind != "hub"
        && access.accepted()
    {
        return store::automatic::configure(conn, device, access).await;
    }
    Ok(false)
}
