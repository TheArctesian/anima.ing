//! The device's session limit (RA-036). Only `start` writes session rows, so files tunnels
//! never count.

use sqlx::PgConnection;

use crate::{
    fleet::{
        devices,
        sessions::{Conflict, Error, device_error, store},
    },
    ids::DeviceId,
};

/// On its own this only answers early: two starts can both pass it.
pub(super) async fn check(conn: &mut PgConnection, device: DeviceId) -> Result<(), Error> {
    let limit = devices::session_limit(&mut *conn, device)
        .await
        .map_err(|error| device_error(device, error))?;
    let Some(limit) = limit else {
        return Ok(());
    };
    let live = store::live_count(conn, device).await?;
    if live >= i64::from(limit.get()) {
        return Err(Error::Conflict(Conflict::SessionLimit));
    }
    Ok(())
}

/// The count reads after the start lock is taken, so under read committed the row of the start
/// that held the lock is counted.
pub(super) async fn check_locked(conn: &mut PgConnection, device: DeviceId) -> Result<(), Error> {
    store::lock_starts(&mut *conn, device).await?;
    check(conn, device).await
}
