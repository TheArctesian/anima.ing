//! The only SQL for this domain's tables (CQ-032); rows hold sealed values only (SEC-003).
//! Every query takes the kinds it may match, so a caller sees and removes only its own kinds.
//! The advisory lock is per device, not per kind, so writers of every kind share one lock order.

use std::collections::HashSet;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::ids::{DeviceId, SourceAccountId, SourceSecretId};

use super::{Error, Sealed};

const DEVICE_FKEY: &str = "secrets_device_id_fkey";

#[derive(sqlx::FromRow)]
pub(super) struct StoredSecret {
    pub(super) kind: String,
    pub(super) nonce: Vec<u8>,
    pub(super) ciphertext: Vec<u8>,
    pub(super) key_id: String,
    /// The host of a row sealed before its host was sealed with it (SEC-011).
    pub(super) legacy_host: Option<String>,
}

pub(super) async fn lock_device(conn: &mut PgConnection, device: DeviceId) -> Result<(), Error> {
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('credentials.secrets:' || $1::text, 0))",
    )
    .bind(device)
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn delete_kinds(
    conn: &mut PgConnection,
    device: DeviceId,
    kinds: &[&str],
) -> Result<u64, Error> {
    let done =
        sqlx::query("DELETE FROM credentials.secrets WHERE device_id = $1 AND kind = ANY($2)")
            .bind(device)
            .bind(kinds)
            .execute(conn)
            .await?;
    Ok(done.rows_affected())
}

/// No `legacy_host`: the sealed value holds its host (SEC-011).
pub(super) async fn insert(
    conn: &mut PgConnection,
    device: DeviceId,
    kind: &str,
    sealed: &Sealed,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO credentials.secrets (id, device_id, kind, nonce, ciphertext, key_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(device)
    .bind(kind)
    .bind(sealed.nonce())
    .bind(sealed.ciphertext())
    .bind(sealed.key_id())
    .execute(conn)
    .await
    .map_err(|error| {
        let constraint = error.as_database_error().and_then(|e| e.constraint());
        if constraint == Some(DEVICE_FKEY) {
            Error::UnknownDevice
        } else {
            Error::Database(error)
        }
    })?;
    Ok(())
}

pub(super) async fn fetch_kinds(
    db: &PgPool,
    device: DeviceId,
    kinds: &[&str],
) -> Result<Vec<StoredSecret>, Error> {
    Ok(sqlx::query_as(
        "SELECT kind, nonce, ciphertext, key_id, legacy_host FROM credentials.secrets \
         WHERE device_id = $1 AND kind = ANY($2)",
    )
    .bind(device)
    .bind(kinds)
    .fetch_all(db)
    .await?)
}

pub(super) async fn exists_of_kinds(
    db: &PgPool,
    device: DeviceId,
    kinds: &[&str],
) -> Result<bool, Error> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM credentials.secrets \
         WHERE device_id = $1 AND kind = ANY($2))",
    )
    .bind(device)
    .bind(kinds)
    .fetch_one(db)
    .await?)
}

pub(super) async fn devices_with_kinds(
    db: &PgPool,
    devices: &[DeviceId],
    kinds: &[&str],
) -> Result<HashSet<DeviceId>, Error> {
    let found: Vec<DeviceId> = sqlx::query_scalar(
        "SELECT DISTINCT device_id FROM credentials.secrets \
         WHERE device_id = ANY($1) AND kind = ANY($2)",
    )
    .bind(devices)
    .bind(kinds)
    .fetch_all(db)
    .await?;
    Ok(found.into_iter().collect())
}

#[derive(sqlx::FromRow)]
pub(super) struct StoredSourceSecret {
    pub(super) nonce: Vec<u8>,
    pub(super) ciphertext: Vec<u8>,
    pub(super) key_id: String,
}

/// One statement, so concurrent stores for one account leave exactly one row.
pub(super) async fn upsert_source(
    conn: &mut PgConnection,
    account: SourceAccountId,
    sealed: &Sealed,
) -> Result<SourceSecretId, Error> {
    Ok(sqlx::query_scalar(
        "INSERT INTO credentials.source_secrets (id, account_id, nonce, ciphertext, key_id) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (account_id) DO UPDATE SET nonce = EXCLUDED.nonce, \
         ciphertext = EXCLUDED.ciphertext, key_id = EXCLUDED.key_id \
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(account)
    .bind(sealed.nonce())
    .bind(sealed.ciphertext())
    .bind(sealed.key_id())
    .fetch_one(conn)
    .await?)
}

pub(super) async fn fetch_source(
    db: &PgPool,
    id: SourceSecretId,
) -> Result<Option<StoredSourceSecret>, Error> {
    Ok(sqlx::query_as(
        "SELECT nonce, ciphertext, key_id FROM credentials.source_secrets WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

/// A reference pointed at another account's row deletes nothing.
pub(super) async fn delete_source(
    conn: &mut PgConnection,
    (id, account): (SourceSecretId, SourceAccountId),
) -> Result<bool, Error> {
    let done =
        sqlx::query("DELETE FROM credentials.source_secrets WHERE id = $1 AND account_id = $2")
            .bind(id)
            .bind(account)
            .execute(conn)
            .await?;
    Ok(done.rows_affected() > 0)
}
