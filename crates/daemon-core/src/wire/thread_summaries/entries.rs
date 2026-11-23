//! The daemon's resumable transcript upload (K8 Section 4.2, TH21).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ThreadSummary;

pub const MAX_THREAD_UPLOAD: usize = 512 * 1024;
pub const MAX_THREAD_ENTRY: usize = 32 * 1024;
pub const MAX_THREAD_ENTRIES: usize = 12;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUpload {
    pub device: Uuid,
    pub summary: ThreadSummary,
    pub entries: Vec<ThreadEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ThreadEntry {
    pub sequence: i64,
    pub content: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUploadReceipt {
    pub id: Uuid,
    pub last_sequence: i64,
}

impl ThreadUpload {
    #[must_use]
    pub fn fits(&self) -> bool {
        self.summary.fits()
            && (1..=MAX_THREAD_ENTRIES).contains(&self.entries.len())
            && self.entries.iter().all(ThreadEntry::fits)
            && self
                .entries
                .windows(2)
                .all(|pair| pair[0].sequence + 1 == pair[1].sequence)
    }
}

impl ThreadEntry {
    fn fits(&self) -> bool {
        (1..=(1_i64 << 53) - 1).contains(&self.sequence)
            && self.content.is_object()
            && can_store(&self.content)
            && serde_json::to_vec(&self.content).is_ok_and(|wire| wire.len() <= MAX_THREAD_ENTRY)
    }
}

// PostgreSQL JSONB cannot store a NUL in a string or in an object key.
fn can_store(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => !text.contains('\0'),
        serde_json::Value::Array(values) => values.iter().all(can_store),
        serde_json::Value::Object(fields) => fields
            .iter()
            .all(|(key, value)| !key.contains('\0') && can_store(value)),
        _ => true,
    }
}
