//! Revoke ended or refused checkout credentials without holding a lease or transaction.
use chrono::TimeDelta;
use futures_util::{StreamExt as _, stream};
use tokio::time::{MissedTickBehavior, interval};

use super::{Error, checkout, jobs};
use crate::{
    engine::sources::{Scheduling, revoke_repository_checkout},
    fleet::credentials::Sealed,
};

/// Run one bounded checkout revocation batch. A failed revocation can retry after the delay.
///
/// # Errors
/// The Hub cannot read or update the durable revocation queue.
pub async fn revoke_due(sources: &Scheduling) -> Result<(), Error> {
    let now = sources.clock.now();
    let entries = checkout::due(&sources.pool, (now, now + TimeDelta::minutes(1))).await?;
    stream::iter(entries)
        .for_each_concurrent(4, |row| async move {
            if revoke(sources, &row).await.is_err() {
                tracing::warn!("repository checkout credential revocation will retry");
            }
        })
        .await;
    super::workspace::revoke_due(sources).await
}

async fn revoke(sources: &Scheduling, row: &checkout::Revocation) -> Result<(), Error> {
    let context = checkout::seal_context(row.job, row.attempt, row.device);
    let sealed = Sealed::from_bytes(&row.sealed).map_err(|_| Error::Unreadable)?;
    let value = sources
        .vault
        .open(&sealed, context.as_bytes())
        .map_err(|_| Error::Unreadable)?;
    revoke_repository_checkout(&sources.transport, &value)
        .await
        .map_err(|_| Error::Unreadable)?;
    checkout::revoked(&sources.pool, row).await
}

/// Periodically retry checkout credential revocation, independently of job lease renewal.
#[must_use]
pub fn spawn(sources: Scheduling) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = interval(jobs::LEASE_SWEEP);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            if revoke_due(&sources).await.is_err() {
                tracing::warn!("the repository checkout revocation queue could not be read");
            }
        }
    })
}
