//! The operator's controls on an account (ITG-003, ITG-004; K-E1 Section 10).

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::{Error, intake::Registry, model::Account, store};
use crate::{fleet::credentials::clear_source_secret, ids::SourceAccountId};

/// Whether "Sync now" starts a run.
///
/// # Errors
/// `Paused` or `NoConsumer` (`CONFLICT` at the API).
pub fn sync_now(account: &Account, consumer_on: bool, now: DateTime<Utc>) -> Result<bool, Error> {
    if account.paused_at.is_some() {
        Err(Error::Paused)
    } else if !consumer_on {
        Err(Error::NoConsumer)
    } else {
        Ok(!account.syncing(now))
    }
}

/// Resuming makes the account due at `now`; returns the account before and after.
///
/// # Errors
/// `NotFound` when there is no such account; `Database` when a write fails.
pub async fn set_paused(
    conn: &mut PgConnection,
    id: SourceAccountId,
    (paused, now): (bool, DateTime<Utc>),
) -> Result<(Account, Account), Error> {
    let before = store::lock(conn, id).await?.ok_or(Error::NotFound)?;
    let after = store::set_paused(conn, id, (paused, now))
        .await?
        .ok_or(Error::NotFound)?;
    Ok((before, after))
}

/// Deletes the account, its runs and its sealed key. With `delete_records` each intake deletes
/// its records, extension on or not (ITG-004). A run that holds the account loses its claim.
///
/// # Errors
/// `NotFound`; `Intake`, `Credentials` or `Database` when a step fails, and the caller rolls back.
pub async fn remove(
    conn: &mut PgConnection,
    registry: &Registry,
    id: SourceAccountId,
    delete_records: bool,
) -> Result<(Account, u64), Error> {
    let account = store::lock(conn, id).await?.ok_or(Error::NotFound)?;
    let mut deleted = 0;
    if delete_records {
        for intake in registry.intakes_of(&account.source) {
            deleted += intake.forget(conn, id).await?;
        }
    }
    store::delete(conn, id).await?;
    if let Some(credential) = account.credential_id {
        clear_source_secret(conn, (credential, id)).await?;
    }
    Ok((account, deleted))
}
