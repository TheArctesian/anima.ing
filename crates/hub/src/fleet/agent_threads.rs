//! Thread summaries from enrolled devices (TH21).

mod store;

use daemon_core::wire::{ThreadSummaries, ThreadSummariesReceipt};
use sqlx::PgConnection;

use crate::ids::DeviceId;

/// Records announced summaries while the caller holds the device row lock.
///
/// # Errors
/// Returns a database error if the summaries cannot be recorded.
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
