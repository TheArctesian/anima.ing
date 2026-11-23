use daemon_core::wire::{ThreadUpload, ThreadUploadReceipt};
use sqlx::PgConnection;

use super::store;
use crate::ids::DeviceId;

/// Appends an upload while its authenticated device is locked. A gap leaves the caller's
/// transaction uncommitted. A retry acknowledges the sequence the hub already holds.
pub(in crate::fleet) async fn append(
    tx: &mut PgConnection,
    device: DeviceId,
    upload: &ThreadUpload,
) -> Result<Option<ThreadUploadReceipt>, sqlx::Error> {
    let Some(first) = upload.entries.first() else {
        return Ok(None);
    };
    let receipt = store::record_summary(tx, device, &upload.summary).await?;
    let mut last_sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(sequence), 0) FROM agent_threads.entries WHERE thread_id = $1",
    )
    .bind(receipt.id)
    .fetch_one(&mut *tx)
    .await?;
    if first.sequence > last_sequence + 1 {
        return Ok(None);
    }
    for entry in &upload.entries {
        if entry.sequence <= last_sequence {
            continue;
        }
        sqlx::query(
            "INSERT INTO agent_threads.entries (thread_id, sequence, content) VALUES ($1, $2, $3)",
        )
        .bind(receipt.id)
        .bind(entry.sequence)
        .bind(&entry.content)
        .execute(&mut *tx)
        .await?;
        last_sequence = entry.sequence;
    }
    Ok(Some(ThreadUploadReceipt {
        id: receipt.id,
        last_sequence,
    }))
}
