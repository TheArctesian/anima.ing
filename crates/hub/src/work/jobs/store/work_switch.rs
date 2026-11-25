//! The hub work switch of a machine (USE-004, R-270).

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::super::{Error, placement};
use super::machines;
use crate::ids::DeviceId;

/// Checks both `WORK_OFF` causes and holds the machine's work state until commit.
/// The caller takes the device lock first, including for a machine with no claim row.
///
/// # Errors
/// Returns a store error if the work state cannot be read.
pub async fn blocks_dispatch(
    conn: &mut PgConnection,
    device: DeviceId,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    Ok(machines::machine_locked(conn, device)
        .await?
        .is_some_and(|machine| placement::is_off(&machine, at)))
}

/// Changes the operator's switch while preserving the daemon's switches and cooldown.
/// The caller takes the device lock before invoking this function.
///
/// # Errors
/// Returns a store error if the switch cannot be saved.
pub async fn set_device_work(
    conn: &mut PgConnection,
    device: DeviceId,
    on: bool,
    at: DateTime<Utc>,
) -> Result<(), Error> {
    machines::set_device_work(conn, device, on, at).await
}
