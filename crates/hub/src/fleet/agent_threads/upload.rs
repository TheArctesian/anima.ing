use daemon_core::wire::{ThreadUpload, ThreadUploadReceipt};
use sqlx::PgConnection;

use super::store;
use crate::ids::DeviceId;

/// The caller holds the device's lock; a gap returns `None` and a retry acknowledges the held sequence.
pub(in crate::fleet) async fn append(
    tx: &mut PgConnection,
    device: DeviceId,
    upload: &ThreadUpload,
) -> Result<Option<ThreadUploadReceipt>, sqlx::Error> {
    let Some(first) = upload.entries.first() else {
        return Ok(None);
    };
    let receipt = store::record_summary(tx, device, &upload.summary).await?;
    let mut last_sequence = store::last_sequence(tx, receipt.id).await?;
    if first.sequence > last_sequence + 1 {
        return Ok(None);
    }
    for entry in &upload.entries {
        if entry.sequence <= last_sequence {
            continue;
        }
        store::insert_entry(tx, receipt.id, entry).await?;
        last_sequence = entry.sequence;
    }
    Ok(Some(ThreadUploadReceipt {
        id: receipt.id,
        last_sequence,
    }))
}
