//! The device policy of a compute node (CMP-002, CAP-010 to CAP-013): its limits, whether it
//! takes model work now and the drain when it stops. The switch "takes work" is the one of the
//! work route (USE-004, OD-C8, R-165); this domain reads it and adds no second switch.
//!
//! Pure: `rules`, `verdict`, `drain`. I/O: `store`, and the join of [`statuses`].

mod drain;
mod rules;
mod store;
#[cfg(test)]
mod tests;
mod verdict;

use chrono::{DateTime, Utc};
use daemon_core::wire::Switch;
use sqlx::{PgConnection, PgPool};

pub use drain::{DRAIN_LIMIT, Drain};
pub use rules::{
    DEFAULT_DISK_SHARE, DEFAULT_MEMORY_SHARE, DevicePolicy, PolicyError, QuietHours, quiet_hours,
};
pub use store::StoreError;
pub use verdict::{NodeState, Stop, WorkSwitch, takes_model_work};

use crate::{
    fleet::{devices, metrics},
    ids::DeviceId,
    work::jobs::{self, Machine},
};

/// Why the policies or their statuses cannot be read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the switch of the machines cannot be read")]
    Jobs(#[from] jobs::Error),
    #[error("the hardware values cannot be read")]
    Metrics(#[from] metrics::Error),
}

/// The policy of a device and whether it takes model work now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicyStatus {
    pub device: DeviceId,
    pub policy: DevicePolicy,
    /// `false` when the device has the default policy.
    pub own: bool,
    pub verdict: Result<(), Stop>,
}

/// The switch of `machine` at `now`, as the policy reads it.
#[must_use]
pub fn switch_of(machine: Option<&Machine>, now: DateTime<Utc>) -> WorkSwitch {
    match machine {
        None => WorkSwitch::Unknown,
        Some(found) if found.switch == Switch::Off => WorkSwitch::OffAtMachine,
        Some(found) if found.held_off(now) => WorkSwitch::OffInHub,
        Some(_) => WorkSwitch::On,
    }
}

/// The policy and the verdict at `now` of each of `devices`.
///
/// # Errors
/// [`Error`] when a store cannot be read.
pub async fn statuses(
    db: &PgPool,
    devices: &[DeviceId],
    now: DateTime<Utc>,
) -> Result<Vec<PolicyStatus>, Error> {
    let mut own = store::policies(db).await?;
    let machines = jobs::machines(db).await?;
    let hardware = metrics::hardware(db).await?;
    Ok(devices
        .iter()
        .map(|device| {
            let found = own.remove(device);
            let policy = found.unwrap_or_default();
            let node = hardware
                .get(device)
                .map_or_else(NodeState::default, |(values, _)| NodeState::from(values));
            let switch = switch_of(machines.get(device), now);
            PolicyStatus {
                device: *device,
                policy,
                own: found.is_some(),
                verdict: takes_model_work(&policy, switch, node, now),
            }
        })
        .collect())
}

/// The policy gate passed to the work route by the hub router.
pub fn model_budget(
    conn: &mut PgConnection,
    (device, apple_silicon): (DeviceId, bool),
    now: DateTime<Utc>,
) -> std::pin::Pin<
    Box<dyn Future<Output = Result<Option<jobs::ModelBudget>, jobs::Error>> + Send + '_>,
> {
    Box::pin(async move {
        budget(conn, (device, apple_silicon), now)
            .await
            .map_err(|error| {
                tracing::error!(%error, %device, "could not read model admission policy");
                jobs::Error::ModelPolicy
            })
    })
}

async fn budget(
    conn: &mut PgConnection,
    (device, apple_silicon): (DeviceId, bool),
    now: DateTime<Utc>,
) -> Result<Option<jobs::ModelBudget>, Error> {
    let policy = store::policy_for(conn, device).await?.unwrap_or_default();
    let hardware = metrics::hardware_for(conn, device).await?;
    let node = hardware
        .as_ref()
        .map_or_else(NodeState::default, NodeState::from);
    if takes_model_work(&policy, WorkSwitch::On, node, now).is_err() {
        return Ok(None);
    }
    let memory = hardware.as_ref().map_or(0, |node| {
        let fallback = if apple_silicon || node.unified_memory_bytes.is_some() {
            0
        } else {
            node.system_memory_bytes
        };
        node.usable_memory.map_or(fallback, |memory| memory.bytes)
    });
    let disk = hardware.and_then(|node| node.free_disk_bytes).unwrap_or(0);
    Ok(Some(jobs::ModelBudget {
        memory_bytes: share(memory, policy.memory_share),
        disk_bytes: share(disk, policy.disk_share),
    }))
}

fn share(bytes: u64, percent: u8) -> u64 {
    let percent = u64::from(percent);
    (bytes / 100) * percent + (bytes % 100) * percent / 100
}

/// Writes the policy of `device` in the caller's transaction.
///
/// # Errors
/// [`StoreError::NoDevice`] for a device deleted meanwhile.
pub async fn set_policy(
    conn: &mut PgConnection,
    device: DeviceId,
    policy: &DevicePolicy,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    devices::lock(conn, device)
        .await
        .map_err(|error| match error {
            devices::Error::NotFound(_) => StoreError::NoDevice,
            devices::Error::Database(error) => StoreError::Database(error),
            _ => StoreError::Unreadable,
        })?;
    store::write(conn, device, policy, now).await
}
