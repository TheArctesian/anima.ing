//! Operational key retrieval by a consumer's daemon (K-E5 Section 2.1, R-147).

use chrono::{DateTime, Utc};
use daemon_core::wire::{FetchedKey, KeyValue, KeyVersion};
use sqlx::PgPool;
use zeroize::Zeroizing;

use super::{Error, Vault, sealing, store, store::consumers, trail};
use crate::{
    fleet::{
        credentials,
        events::{self, AuditAction, AuditTarget, ClientAddr, NewAudit, Operator},
    },
    ids::DeviceId,
};

/// Audited; a missing key, no active version and a non-consumer are one `None` (K-E5 Section 2.1).
///
/// # Errors
/// `Sealing` when the stored value does not open, `Database`, `Trail`.
pub async fn fetch(
    db: &PgPool,
    vault: &Vault,
    (device, name): (DeviceId, &str),
    (now, client): (DateTime<Utc>, ClientAddr),
) -> Result<Option<FetchedKey>, Error> {
    let mut tx = db.begin().await?;
    let Some(found) = store::fetchable(&mut tx, (device, name), now).await? else {
        return Ok(None);
    };
    let secret = sealing::open(vault, found.version, &found.sealed).map_err(Error::Sealing)?;
    let value = KeyValue::new(Zeroizing::new(secret.expose().to_vec()))
        .ok_or(Error::Sealing(credentials::Error::Malformed))?;
    drop(secret);
    consumers::mark_fetched(&mut tx, (found.key, device), (found.number, now)).await?;
    let entry = NewAudit {
        operator: Operator::hub(),
        action: AuditAction::Fetch,
        target: AuditTarget::OperationalKey(found.key),
        client,
        change: trail::fetched(name, found.number, device),
    };
    events::audit(&mut tx, now, entry).await?;
    tx.commit().await?;
    Ok(Some(FetchedKey {
        name: name.to_owned(),
        version: version_number(found.number),
        value,
    }))
}

/// # Errors
/// `Database` when the read fails.
pub async fn current_for(
    db: &PgPool,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Vec<KeyVersion>, Error> {
    let mut conn = db.acquire().await?;
    let rows = consumers::current_for(&mut conn, device, now).await?;
    Ok(rows
        .into_iter()
        .map(|(name, number)| KeyVersion {
            name,
            version: version_number(number),
        })
        .collect())
}

/// A key the daemon does not name holds no version (K-E5 Section 2.1).
///
/// # Errors
/// `Database` when the write fails.
pub async fn record_held(
    db: &PgPool,
    device: DeviceId,
    held: &[KeyVersion],
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let names: Vec<String> = held.iter().map(|one| one.name.clone()).collect();
    let versions: Vec<i32> = held
        .iter()
        .map(|one| i32::try_from(one.version).unwrap_or(i32::MAX))
        .collect();
    let mut conn = db.acquire().await?;
    consumers::record_held(&mut conn, device, (&names, &versions), now).await?;
    Ok(())
}

/// The column's check keeps the number at least 1.
fn version_number(number: i32) -> u32 {
    u32::try_from(number).unwrap_or_default()
}
