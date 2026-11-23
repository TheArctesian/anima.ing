//! Operational keys: the credentials and runtime secrets a server of the operator needs (R-147;
//! K-E5 Section 2; SEC-003).
//!
//! A value is sealed before storage and leaves the hub only through [`fetch`], to a consumer's
//! daemon. No metadata, audit entry, event, error text or `Debug` output holds a value (LOG-007).
//! Each change writes its rows, one audit entry and one event in the caller's transaction.

mod lifecycle;
mod model;
mod removed_device;
mod retrieval;
mod rules;
mod sealing;
mod store;
mod trail;

#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};

pub use model::{Consumer, Due, Ending, Grant, Metadata, OperationalKey, Version, VersionState};
pub use removed_device::forget_device;
pub use retrieval::{current_for, fetch, record_held};
pub use rules::{NAME_RULE, Problem, WORKLOAD_RULE};

use super::{Secret, Vault};
use crate::{
    fleet::events::{self, Trail},
    ids::OperationalKeyId,
};
use store::{
    NewVersion,
    consumers::{self, Granted},
    reads,
};

/// Messages are fixed text: no value, no name and no SQL (LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the operational key store is unavailable")]
    Database(#[from] sqlx::Error),
    #[error("the audit log or the event log cannot be written")]
    Trail(#[from] events::Error),
    #[error("an operational key cannot be sealed or opened")]
    Sealing(#[source] super::Error),
    #[error("a field of the operational key is not valid")]
    Invalid(Vec<Problem>),
    #[error("an operational key has this name already")]
    NameTaken,
    #[error("no operational key has this name")]
    UnknownKey,
    #[error("the device does not exist")]
    UnknownDevice,
    #[error("the key is not granted to this device for this workload")]
    NoGrant,
    /// A new value would read as expired at once; the operator sets a later expiry first.
    #[error("the key passed its expiry")]
    Expired,
    #[error("the key has no active version")]
    NoActiveVersion,
}

/// No `Debug`: it holds the value.
pub struct NewKey {
    pub name: String,
    pub metadata: Metadata,
    pub value: Secret,
}

/// # Errors
/// `Database` when the read fails.
pub async fn keys(db: &PgPool, now: DateTime<Utc>) -> Result<Vec<OperationalKey>, Error> {
    let mut conn = db.acquire().await?;
    Ok(reads::keys(&mut conn, None, now).await?)
}

/// # Errors
/// `Database` when the read fails.
pub async fn key(
    db: &PgPool,
    name: &str,
    now: DateTime<Utc>,
) -> Result<Option<OperationalKey>, Error> {
    let mut conn = db.acquire().await?;
    Ok(reads::keys(&mut conn, Some(name), now).await?.pop())
}

/// # Errors
/// `Invalid`, `NameTaken` (a removed key keeps its name), `Sealing`, `Database`, `Trail`.
pub async fn create(
    conn: &mut PgConnection,
    vault: &Vault,
    new: NewKey,
    trail: Trail<'_>,
) -> Result<OperationalKey, Error> {
    let mut problems = Vec::new();
    if !rules::is_name(&new.name) {
        problems.push(Problem {
            field: "name",
            message: NAME_RULE,
        });
    }
    problems.extend(rules::problems(&new.metadata));
    problems.extend(rules::value_problem(new.value.expose().len()));
    if !problems.is_empty() {
        return Err(Error::Invalid(problems));
    }
    let (now, by) = (trail.now, trail.operator().to_string());
    let id: OperationalKeyId = store::new_id();
    if !store::insert_key(conn, (id, &new.name), &new.metadata, (now, &by)).await? {
        return Err(Error::NameTaken);
    }
    add_version(conn, vault, (id, new.value, false), (now, &by)).await?;
    trail
        .write(conn, trail::created(id, &new.name, &new.metadata))
        .await?;
    read_back(conn, &new.name, now).await
}

/// # Errors
/// `UnknownKey`, `Invalid`, `Database`, `Trail`.
pub async fn update(
    conn: &mut PgConnection,
    name: &str,
    metadata: Metadata,
    trail: Trail<'_>,
) -> Result<OperationalKey, Error> {
    let problems = rules::problems(&metadata);
    if !problems.is_empty() {
        return Err(Error::Invalid(problems));
    }
    let id = locked(conn, name).await?;
    let now = trail.now;
    // An expiry already passed ends the version before a later expiry could revive it.
    store::settle_expired(conn, Some(id), now).await?;
    store::update_metadata(conn, id, &metadata).await?;
    trail
        .write(conn, trail::updated(id, name, &metadata))
        .await?;
    read_back(conn, name, now).await
}

/// Supersedes the active version at once (R-147); `rotated_at_provider` only records what the
/// operator said, and the hub changes nothing at the provider.
///
/// # Errors
/// `UnknownKey`, `Invalid` for a bad value length, `Expired`, `Sealing`, `Database`, `Trail`.
pub async fn rotate(
    conn: &mut PgConnection,
    vault: &Vault,
    (name, value, rotated_at_provider): (&str, Secret, bool),
    trail: Trail<'_>,
) -> Result<Ending, Error> {
    if let Some(problem) = rules::value_problem(value.expose().len()) {
        return Err(Error::Invalid(vec![problem]));
    }
    let id = locked(conn, name).await?;
    let (now, by) = (trail.now, trail.operator().to_string());
    store::settle_expired(conn, Some(id), now).await?;
    if store::is_expired(conn, id, now).await? {
        return Err(Error::Expired);
    }
    let ended = store::end_active(conn, id, (VersionState::Superseded, now)).await?;
    let made = add_version(conn, vault, (id, value, rotated_at_provider), (now, &by)).await?;
    let entry = trail::rotated((id, name), (ended, made), rotated_at_provider);
    trail.write(conn, entry).await?;
    ending(conn, (id, name, ended), now).await
}

/// # Errors
/// `UnknownKey`, `NoActiveVersion` when no version is active, `Database`, `Trail`.
pub async fn revoke(
    conn: &mut PgConnection,
    name: &str,
    trail: Trail<'_>,
) -> Result<Ending, Error> {
    let id = locked(conn, name).await?;
    let now = trail.now;
    store::settle_expired(conn, Some(id), now).await?;
    let ended = store::end_active(conn, id, (VersionState::Revoked, now)).await?;
    if ended.is_none() {
        return Err(Error::NoActiveVersion);
    }
    let holders = holders_of(conn, id, ended).await?;
    trail
        .write(conn, trail::revoked((id, name), ended, holders.len()))
        .await?;
    ending(conn, (id, name, ended), now).await
}

/// Ends the active version and its retrieval; the name stays taken.
///
/// # Errors
/// `UnknownKey`, `Database`, `Trail`.
pub async fn remove(
    conn: &mut PgConnection,
    name: &str,
    trail: Trail<'_>,
) -> Result<Ending, Error> {
    let id = locked(conn, name).await?;
    let now = trail.now;
    store::settle_expired(conn, Some(id), now).await?;
    let ended = store::end_active(conn, id, (VersionState::Revoked, now)).await?;
    let ending = ending(conn, (id, name, ended), now).await?;
    store::mark_removed(conn, id, now).await?;
    let entry = trail::removed((id, name), ended, ending.still_held_by.len());
    trail.write(conn, entry).await?;
    Ok(ending)
}

/// A trust decision of the operator; a grant that exists writes nothing.
///
/// # Errors
/// `UnknownKey`, `Invalid` for a workload that is not a label, `UnknownDevice`, `Database`,
/// `Trail`.
pub async fn grant(
    conn: &mut PgConnection,
    grant: Grant,
    trail: Trail<'_>,
) -> Result<OperationalKey, Error> {
    workload_fits(&grant.workload)?;
    let id = locked(conn, &grant.name).await?;
    let (now, by) = (trail.now, trail.operator().to_string());
    let row = (id, grant.device, grant.workload.as_str());
    match consumers::insert(conn, row, (now, &by)).await? {
        Granted::Made => trail.write(conn, trail::granted(id, &grant)).await?,
        Granted::Kept => {}
        Granted::NoDevice => return Err(Error::UnknownDevice),
    }
    read_back(conn, &grant.name, now).await
}

/// The daemon drops the key at its next report.
///
/// # Errors
/// `UnknownKey`, `NoGrant`, `Database`, `Trail`.
pub async fn ungrant(
    conn: &mut PgConnection,
    grant: Grant,
    trail: Trail<'_>,
) -> Result<OperationalKey, Error> {
    let id = locked(conn, &grant.name).await?;
    let now = trail.now;
    if !consumers::delete(conn, (id, grant.device, grant.workload.as_str())).await? {
        return Err(Error::NoGrant);
    }
    trail.write(conn, trail::ungranted(id, &grant)).await?;
    read_back(conn, &grant.name, now).await
}

/// Ends expired versions, then empties each sealed value that ended over 30 days ago; how many.
///
/// # Errors
/// `Database` when a write fails.
pub async fn forget_ended(db: &PgPool, now: DateTime<Utc>) -> Result<u64, Error> {
    let mut tx = db.begin().await?;
    store::settle_expired(&mut tx, None, now).await?;
    let emptied = store::forget_ended(&mut tx, lifecycle::forget_before(now)).await?;
    tx.commit().await?;
    Ok(emptied)
}

fn workload_fits(workload: &str) -> Result<(), Error> {
    if rules::is_workload(workload) {
        Ok(())
    } else {
        Err(Error::Invalid(vec![Problem {
            field: "workload",
            message: WORKLOAD_RULE,
        }]))
    }
}

async fn locked(conn: &mut PgConnection, name: &str) -> Result<OperationalKeyId, Error> {
    store::lock_key(conn, name).await?.ok_or(Error::UnknownKey)
}

async fn read_back(
    conn: &mut PgConnection,
    name: &str,
    now: DateTime<Utc>,
) -> Result<OperationalKey, Error> {
    reads::keys(conn, Some(name), now)
        .await?
        .pop()
        .ok_or(Error::UnknownKey)
}

async fn add_version(
    conn: &mut PgConnection,
    vault: &Vault,
    (key, value, rotated_at_provider): (OperationalKeyId, Secret, bool),
    made: store::Made<'_>,
) -> Result<i32, Error> {
    let id = store::new_id();
    let sealed = sealing::seal(vault, id, &value).map_err(Error::Sealing)?;
    drop(value);
    let version = NewVersion {
        id,
        key,
        sealed: &sealed,
        rotated_at_provider,
    };
    Ok(store::insert_version(conn, version, made).await?)
}

async fn holders_of(
    conn: &mut PgConnection,
    key: OperationalKeyId,
    ended: Option<i32>,
) -> Result<Vec<Consumer>, Error> {
    match ended {
        Some(number) => Ok(reads::holders(conn, key, number).await?),
        None => Ok(Vec::new()),
    }
}

async fn ending(
    conn: &mut PgConnection,
    (key, name, ended): (OperationalKeyId, &str, Option<i32>),
    now: DateTime<Utc>,
) -> Result<Ending, Error> {
    Ok(Ending {
        key: read_back(conn, name, now).await?,
        ended,
        still_held_by: holders_of(conn, key, ended).await?,
    })
}
