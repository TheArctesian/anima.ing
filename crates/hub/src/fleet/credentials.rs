//! Device credentials: secrets sealed with the hub key before they reach storage (SEC-003).
//!
//! Plaintext leaves this module only through [`Secret::expose`]; secret types implement no
//! format or serialise traits, and this module writes no logs (LOG-007).
//! Each secret is bound to its device, its kind and the host it was entered for, and opens only
//! for that host (SEC-011). Only sessions read device secrets back; the API sees only whether one
//! exists (SEC-005). Only the source token minter reads a source secret back (SRC-008).
//! Writers of every kind take one advisory lock per device.

pub mod graphql;
mod host;
pub mod operational;
mod password;
mod secret;
mod source;
mod ssh;
mod store;
mod vault;

#[cfg(test)]
mod tests;

/// A fixed throwaway key for tests; never a real key (SEC-004).
#[cfg(test)]
pub(crate) fn test_vault() -> Vault {
    Vault::from_key(zeroize::Zeroizing::new([7_u8; 32]))
}

use std::collections::HashSet;

use sqlx::{Connection as _, PgConnection, PgPool};

use crate::ids::{DeviceId, SourceAccountId, SourceSecretId};

use self::ssh::SshKind;

pub use password::PasswordKind;
pub use secret::Secret;
pub use ssh::SshSecret;
pub use vault::{Sealed, Vault};

/// Why a credential operation failed. Messages name no path, key, nonce or plaintext.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the credential key file cannot be read ({0})")]
    KeyFile(std::io::ErrorKind),
    #[error("the credential key file does not hold a base64-encoded 32-byte key")]
    KeyFormat,
    #[error("the secret was sealed with a different key")]
    WrongKey,
    #[error("the secret cannot be decrypted")]
    Decrypt,
    #[error("the sealed secret is malformed")]
    Malformed,
    /// The source holds the database error, never a secret.
    #[error("the credential store is unavailable")]
    Database(#[from] sqlx::Error),
    #[error("the device does not exist")]
    UnknownDevice,
    #[error("the OS random source failed")]
    Random(#[source] getrandom::Error),
    /// Names neither host (SEC-011, LOG-007).
    #[error("the credential was entered for another host of the device")]
    EnteredForOtherHost,
    #[error("the source secret does not exist")]
    UnknownSourceSecret,
}

/// Replaces every SSH secret of the device, bound to `host` (SEC-011), in a savepoint, so a
/// failure leaves the previous secret in place.
///
/// Lock order: call this before taking `FOR UPDATE` on, or deleting, the device row in the same
/// transaction, or a concurrent store can deadlock (SQLSTATE 40P01).
///
/// # Errors
/// [`Error::Random`], [`Error::UnknownDevice`] or [`Error::Database`].
pub async fn store_ssh(
    conn: &mut PgConnection,
    vault: &Vault,
    (device, host): (DeviceId, &str),
    secret: SshSecret,
) -> Result<(), Error> {
    let sealed = secret
        .into_parts()
        .into_iter()
        .map(|(kind, part)| {
            Ok((
                kind,
                host::seal(vault, (device, host), kind.as_sql(), &part)?,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let mut tx = conn.begin().await?;
    store::lock_device(&mut tx, device).await?;
    store::delete_kinds(&mut tx, device, &SshKind::SQL_KINDS).await?;
    for (kind, value) in &sealed {
        store::insert(&mut tx, device, kind.as_sql(), value).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Opens the device's SSH secret for a connection to `host`; `None` when none is stored.
///
/// # Errors
/// [`Error::EnteredForOtherHost`] for another host (SEC-011); otherwise a row that does not
/// open, or [`Error::Database`].
pub async fn load_ssh(
    db: &PgPool,
    vault: &Vault,
    (device, host): (DeviceId, &str),
) -> Result<Option<SshSecret>, Error> {
    let rows = store::fetch_kinds(db, device, &SshKind::SQL_KINDS).await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let parts = rows
        .into_iter()
        .map(|row| {
            let kind = SshKind::from_sql(&row.kind)?;
            let legacy = ssh::context(device, kind);
            Ok((kind, host::open(vault, row, (device, host), &legacy)?))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    SshSecret::from_parts(parts).map(Some)
}

/// # Errors
/// [`Error::Database`] when the store cannot be read.
pub async fn has_ssh(db: &PgPool, device: DeviceId) -> Result<bool, Error> {
    store::exists_of_kinds(db, device, &SshKind::SQL_KINDS).await
}

/// Which of the given devices have an SSH secret stored, in one query (API-005).
///
/// # Errors
/// [`Error::Database`] when the store cannot be read.
pub async fn with_ssh(db: &PgPool, devices: &[DeviceId]) -> Result<HashSet<DeviceId>, Error> {
    store::devices_with_kinds(db, devices, &SshKind::SQL_KINDS).await
}

/// Returns true if a secret was removed. Same lock order as [`store_ssh`].
///
/// # Errors
/// [`Error::Database`] when the store cannot be written.
pub async fn clear_ssh(conn: &mut PgConnection, device: DeviceId) -> Result<bool, Error> {
    let mut tx = conn.begin().await?;
    store::lock_device(&mut tx, device).await?;
    let removed = store::delete_kinds(&mut tx, device, &SshKind::SQL_KINDS).await?;
    tx.commit().await?;
    Ok(removed > 0)
}

/// Replaces the device's password of `kind`, bound to `host` (SEC-011), in a savepoint; other
/// kinds stay. Same lock order as [`store_ssh`].
///
/// # Errors
/// [`Error::Random`], [`Error::UnknownDevice`] or [`Error::Database`].
pub async fn store_password(
    conn: &mut PgConnection,
    vault: &Vault,
    (device, host): (DeviceId, &str),
    (kind, secret): (PasswordKind, Secret),
) -> Result<(), Error> {
    let sealed = host::seal(vault, (device, host), kind.as_sql(), &secret)?;
    drop(secret);
    let mut tx = conn.begin().await?;
    store::lock_device(&mut tx, device).await?;
    store::delete_kinds(&mut tx, device, &[kind.as_sql()]).await?;
    store::insert(&mut tx, device, kind.as_sql(), &sealed).await?;
    tx.commit().await?;
    Ok(())
}

/// Opens the device's password of `kind` for a connection to `host`; `None` when none is stored.
///
/// # Errors
/// [`Error::EnteredForOtherHost`] for another host (SEC-011); otherwise a row that does not
/// open, or [`Error::Database`].
pub async fn load_password(
    db: &PgPool,
    vault: &Vault,
    (device, host): (DeviceId, &str),
    kind: PasswordKind,
) -> Result<Option<Secret>, Error> {
    // The unique (device_id, kind) constraint allows at most one row.
    let Some(row) = store::fetch_kinds(db, device, &[kind.as_sql()])
        .await?
        .pop()
    else {
        return Ok(None);
    };
    let legacy = password::context(device, kind);
    host::open(vault, row, (device, host), &legacy).map(Some)
}

/// Which of the given devices have a password of `kind` stored, in one query (API-005).
///
/// # Errors
/// [`Error::Database`] when the store cannot be read.
pub async fn with_password(
    db: &PgPool,
    devices: &[DeviceId],
    kind: PasswordKind,
) -> Result<HashSet<DeviceId>, Error> {
    store::devices_with_kinds(db, devices, &[kind.as_sql()]).await
}

/// Returns true if a password was removed. Same lock order as [`store_ssh`].
///
/// # Errors
/// [`Error::Database`] when the store cannot be written.
pub async fn clear_password(
    conn: &mut PgConnection,
    device: DeviceId,
    kind: PasswordKind,
) -> Result<bool, Error> {
    let mut tx = conn.begin().await?;
    store::lock_device(&mut tx, device).await?;
    let removed = store::delete_kinds(&mut tx, device, &[kind.as_sql()]).await?;
    tx.commit().await?;
    Ok(removed > 0)
}

/// Stores the source `account`'s credential bound to the account; storing again replaces the
/// value and keeps the id (SRC-008, PRJ-018).
///
/// # Errors
/// [`Error::Random`] or [`Error::Database`].
pub async fn store_source_secret(
    conn: &mut PgConnection,
    vault: &Vault,
    account: SourceAccountId,
    secret: Secret,
) -> Result<SourceSecretId, Error> {
    let sealed = source::seal(vault, account, &secret)?;
    drop(secret);
    store::upsert_source(conn, account, &sealed).await
}

/// Opens the source secret `id` for `account`, the source account the caller acts for.
///
/// # Errors
/// [`Error::UnknownSourceSecret`]; [`Error::Decrypt`] when it was sealed for another account.
pub async fn load_source_secret(
    db: &PgPool,
    vault: &Vault,
    (id, account): (SourceSecretId, SourceAccountId),
) -> Result<Secret, Error> {
    let row = store::fetch_source(db, id)
        .await?
        .ok_or(Error::UnknownSourceSecret)?;
    source::open(vault, row, account)
}

/// Returns true if it was removed; a secret of another account is never removed.
///
/// # Errors
/// [`Error::Database`] when the store cannot be written.
pub async fn clear_source_secret(
    conn: &mut PgConnection,
    (id, account): (SourceSecretId, SourceAccountId),
) -> Result<bool, Error> {
    store::delete_source(conn, (id, account)).await
}
