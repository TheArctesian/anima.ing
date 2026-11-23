//! Thread summaries from enrolled devices (TH21).

mod store;

use daemon_core::wire::{ThreadSummaries, ThreadSummariesReceipt};
use sqlx::PgConnection;

use crate::ids::DeviceId;

/// Records the summaries while the caller holds the device row lock.
///
/// # Errors
/// Returns a database error if the summaries cannot be recorded.
pub(in crate::fleet) async fn record(
    tx: &mut PgConnection,
    device: DeviceId,
    batch: &ThreadSummaries,
) -> Result<ThreadSummariesReceipt, sqlx::Error> {
    let count_revision = store::record_count(tx, device, batch).await?;
    let mut summaries = Vec::with_capacity(batch.summaries.len());
    for row in &batch.summaries {
        summaries.push(store::record_summary(tx, device, row).await?);
    }
    Ok(ThreadSummariesReceipt {
        count_revision,
        summaries,
    })
}
