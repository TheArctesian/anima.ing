//! The containers of a device, read-only (HOST-001, HOST-010; R-149), by fixed `docker` commands
//! over the hub's SSH link (CHK-012). No table, event or hub log line holds a container name, an
//! image or a log line, and no error holds text from the device (LOG-007).
//! The only operator values that reach a command are a container identifier, rendered from its
//! hex digits, and a number of lines.

pub mod compose_file;
mod detail;
mod docker;
pub mod graphql;
mod model;
mod project;
mod storage;
#[cfg(test)]
mod tests;
mod updates;

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub use self::model::{
    Container, ContainerDetail, ContainerId, ContainerLog, ContainerState, Health, HealthCheck,
    HealthRun, Label, LineCount, LogLine, Mount, NetworkAddress, NotKnown, Process, ProjectState,
    Tail, UpdateState,
};
pub use self::storage::{DiskUsage, Image, Network, Storage, Volume};
pub use self::updates::ImageUpdate;
use crate::{
    fleet::{
        credentials::Vault,
        devices::{self, Device},
        ssh::{self, CommandOutput, FixedCommand, LinkError},
        targets::AddressPolicy,
    },
    ids::DeviceId,
};

const LIST_LIMIT: Duration = Duration::from_secs(10);
const DETAIL_LIMIT: Duration = Duration::from_secs(15);
/// Docker measures each volume.
const STORAGE_LIMIT: Duration = Duration::from_secs(30);
const LOG_LIMIT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
pub struct Reach<'a> {
    pub db: &'a PgPool,
    pub vault: &'a Vault,
    pub policy: &'a AddressPolicy,
}

/// Why the containers of a device or a log could not be read. No variant holds a host, an
/// address, a container name or the device's output (API-010, LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("device {0} not found")]
    NotFound(DeviceId),
    #[error("a container identifier is 12 to 64 lower-case hexadecimal characters")]
    InvalidContainerId,
    #[error("the number of log lines is 1 to 1000")]
    InvalidLineCount,
    #[error("the SSH link to the device failed")]
    Link(#[source] LinkError),
    #[error("docker is not installed on the device")]
    NoDocker,
    #[error("docker on the device did not succeed")]
    DockerFailed,
    #[error("the device has no container with this identifier")]
    NoContainer,
    /// The list is not what `docker ps` writes, or it passed the link's cap.
    #[error("the device's answer cannot be read")]
    Unreadable,
    #[error("device store error")]
    Devices(#[source] devices::Error),
}

/// Running or not, in Docker's order (newest first).
///
/// # Errors
/// `NotFound` for an unknown device; the [`Error`] for the first step that fails.
pub async fn list(
    reach: Reach<'_>,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Vec<Container>, Error> {
    let device = known(reach.db, device).await?;
    let output = run(reach, &device, &docker::list_command(), (LIST_LIMIT, now)).await?;
    docker::containers(&output)
}

/// Both output streams, oldest first.
///
/// # Errors
/// `NotFound` for an unknown device, `NoContainer` when Docker knows no such container; the
/// [`Error`] for the first step that fails.
pub async fn log(
    reach: Reach<'_>,
    device: DeviceId,
    tail: &Tail,
    now: DateTime<Utc>,
) -> Result<ContainerLog, Error> {
    let device = known(reach.db, device).await?;
    let output = run(reach, &device, &docker::log_command(tail), (LOG_LIMIT, now)).await?;
    docker::log(&output)
}

/// Environment variable names only, never a value (HOST-017).
///
/// # Errors
/// `NotFound` for an unknown device, `NoContainer` when Docker knows no such container; the
/// [`Error`] for the first step that fails.
pub async fn detail(
    reach: Reach<'_>,
    device: DeviceId,
    container: &ContainerId,
    now: DateTime<Utc>,
) -> Result<ContainerDetail, Error> {
    let device = known(reach.db, device).await?;
    let command = detail::command(container);
    let output = run(reach, &device, &command, (DETAIL_LIMIT, now)).await?;
    detail::detail(&output)
}

/// # Errors
/// `NotFound` for an unknown device; the [`Error`] for the first step that fails.
pub async fn storage(
    reach: Reach<'_>,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Storage, Error> {
    let device = known(reach.db, device).await?;
    let command = storage::storage_command();
    let output = run(reach, &device, &command, (STORAGE_LIMIT, now)).await?;
    storage::storage(&output)
}

/// # Errors
/// `NotFound` for an unknown device; the [`Error`] for the first step that fails.
pub async fn networks(
    reach: Reach<'_>,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Vec<Network>, Error> {
    let device = known(reach.db, device).await?;
    let command = storage::networks_command();
    let output = run(reach, &device, &command, (LIST_LIMIT, now)).await?;
    storage::networks(&output)
}

/// Compares the device's digest with the same tag at its public registry (HOST-021); it updates
/// nothing.
///
/// # Errors
/// `NotFound` for an unknown device; the [`Error`] for the first step on the device that
/// fails. A registry that is not read is not an error: its containers have the answer
/// "not known".
pub async fn image_updates(
    reach: Reach<'_>,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Vec<ImageUpdate>, Error> {
    let egress = updates::egress::Egress::new(reach.policy.clone());
    image_updates_through(reach, device, &egress, now).await
}

async fn image_updates_through(
    reach: Reach<'_>,
    device: DeviceId,
    egress: &updates::egress::Egress,
    now: DateTime<Utc>,
) -> Result<Vec<ImageUpdate>, Error> {
    let device = known(reach.db, device).await?;
    let command = updates::command();
    let output = run(reach, &device, &command, (LIST_LIMIT, now)).await?;
    Ok(updates::compare(updates::found(&output)?, egress).await)
}

async fn known(db: &PgPool, device: DeviceId) -> Result<Device, Error> {
    devices::get(db, device).await.map_err(|error| match error {
        devices::Error::NotFound(_) => Error::NotFound(device),
        other => Error::Devices(other),
    })
}

async fn run(
    reach: Reach<'_>,
    device: &Device,
    command: &FixedCommand,
    (limit, now): (Duration, DateTime<Utc>),
) -> Result<CommandOutput, Error> {
    let mut link = ssh::link(reach.db, reach.vault, reach.policy, device, now)
        .await
        .map_err(Error::Link)?;
    link.run(command, limit).await.map_err(Error::Link)
}
