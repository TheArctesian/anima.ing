//! Class A records that a client seals with the vault key (K-E5 Section 3; VLT-001, VLT-002,
//! VLT-006). The hub cannot open them: this module takes no vault key and imports no hub key.

pub mod graphql;
mod store;
mod trail;

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    engine::records::Reference,
    ids::{KeyOfferId, SealedRecordId},
};

/// The most bytes of `sealed` and of `sealed_index`.
pub const SEALED_MAX: usize = 1_048_576;
/// The most bytes of a key offer.
pub const OFFER_MAX: usize = 4096;
/// The bytes of `key_id`: the SHA-256 of the vault key.
pub const KEY_ID_LEN: usize = 32;
pub const OFFER_LIFE: TimeDelta = TimeDelta::minutes(5);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored sealed record is not valid")]
    Stored,
}

/// No `Debug`: the bytes are not for a log.
#[derive(Clone, PartialEq, Eq)]
pub struct Record {
    pub id: SealedRecordId,
    pub owner: Reference,
    pub extension: String,
    pub sealed: Vec<u8>,
    pub sealed_index: Option<Vec<u8>>,
    pub key_id: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NewRecord {
    pub owner: Reference,
    pub extension: String,
    pub sealed: Vec<u8>,
    pub sealed_index: Option<Vec<u8>>,
    pub key_id: Vec<u8>,
}

#[must_use]
pub fn is_extension(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && key.len() <= 40
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// # Errors
/// `Database`, `Stored`.
pub async fn stored(conn: &mut PgConnection, owner: &Reference) -> Result<Option<Record>, Error> {
    store::read(conn, owner).await
}

/// Inserts or replaces the owner's record; `true` when the record is new.
///
/// # Errors
/// `Database`, `Stored`.
pub async fn keep(
    conn: &mut PgConnection,
    new: &NewRecord,
    now: DateTime<Utc>,
) -> Result<(Record, bool), Error> {
    let id = SealedRecordId::from(Uuid::now_v7());
    store::upsert(conn, id, new, now).await
}

/// # Errors
/// `Database`, `Stored`.
pub async fn remove(conn: &mut PgConnection, owner: &Reference) -> Result<Option<Record>, Error> {
    store::delete(conn, owner).await
}

/// Each offer that ended is deleted.
///
/// # Errors
/// `Database`.
pub async fn offer(
    conn: &mut PgConnection,
    sealed: &[u8],
    now: DateTime<Utc>,
) -> Result<KeyOfferId, Error> {
    let id = KeyOfferId::from(Uuid::now_v7());
    store::insert_offer(conn, id, sealed, (now, now + OFFER_LIFE)).await?;
    Ok(id)
}

/// The read deletes the offer; `None` for an offer that was read, that ended, or that never was.
///
/// # Errors
/// `Database`.
pub async fn take_offer(
    conn: &mut PgConnection,
    id: KeyOfferId,
    now: DateTime<Utc>,
) -> Result<Option<Vec<u8>>, Error> {
    Ok(store::take_offer(conn, id, now).await?)
}
