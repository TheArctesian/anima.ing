//! A server keeps the key values it fetched, so removing its device names those keys for
//! revocation (K-E5 Section 2.1).

use sqlx::PgConnection;

use super::{Error, store::consumers, trail};
use crate::{fleet::events::TrailEntry, ids::DeviceId};

/// Ends every grant of `device` in the caller's transaction and returns one trail entry for each
/// key its daemon fetched or held, naming the key and the version, never the value.
///
/// # Errors
/// `Database`.
pub async fn forget_device(
    conn: &mut PgConnection,
    device: DeviceId,
) -> Result<Vec<TrailEntry>, Error> {
    let had = consumers::delete_of_device(conn, device).await?;
    Ok(had
        .into_iter()
        .map(|(key, name, version)| trail::device_removed((key, &name), version, device))
        .collect())
}
