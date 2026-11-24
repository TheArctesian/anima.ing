//! Thread summaries from enrolled devices (TH21).

pub mod graphql;
mod model;
mod store;
mod upload;

pub(in crate::fleet) use upload::append;

use daemon_core::wire::{ThreadSummaries, ThreadSummariesReceipt};
use sqlx::PgConnection;

use crate::ids::DeviceId;

/// The caller holds the device row lock.
pub(in crate::fleet) async fn record_announced(
    tx: &mut PgConnection,
    device: DeviceId,
    batch: Option<&ThreadSummaries>,
) -> Result<Option<ThreadSummariesReceipt>, sqlx::Error> {
    let Some(batch) = batch else {
        return Ok(None);
    };
    let count_revision = store::record_count(tx, device, batch).await?;
    let mut summaries = Vec::with_capacity(batch.summaries.len());
    for row in &batch.summaries {
        summaries.push(store::record_summary(tx, device, row).await?);
    }
    Ok(Some(ThreadSummariesReceipt {
        count_revision,
        summaries,
    }))
}
