//! The record of a found copy in `<model-state>/found/<copy>.json` (Section 2.3): what the daemon
//! read at `copy_adopt`, and what each load checks again.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{FoundModel, Tool, is_part};
use crate::model_work::cause::Cause;
use crate::model_work::files::MAX_FILES;
use crate::model_work::machine::is_model_folder;
use crate::model_work::names::is_lower_hex;

/// The most levels of the location of one file below its folder.
pub const MAX_LEVELS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub copy: Uuid,
    pub found: String,
    pub tool: Tool,
    /// The absolute folder of Section 10.2; a load refuses a folder that the walk no longer reads.
    pub root: String,
    pub files: Vec<RecordFile>,
}

/// A file as the daemon read it at `copy_adopt`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordFile {
    pub location: Vec<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub device: u64,
    pub inode: u64,
    pub modified: i64,
}

/// A file as the daemon reads it now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observed<'a> {
    pub size_bytes: u64,
    pub sha256: &'a str,
}

impl RecordFile {
    /// # Errors
    /// `copy_changed` when the size or the SHA-256 is not that of the record (Section 2.3).
    pub fn holds(&self, observed: Observed<'_>) -> Result<(), Cause> {
        (observed.size_bytes == self.size_bytes && observed.sha256 == self.sha256)
            .then_some(())
            .ok_or(Cause::CopyChanged)
    }
}

impl Record {
    /// Whether a record that the daemon reads back has the form that it writes: the state folder
    /// is writable by the user of the daemon, so it is read as untrusted.
    #[must_use]
    pub fn holds_form(&self) -> bool {
        is_lower_hex(&self.found, 64)
            && is_model_folder(&self.root)
            && (1..=MAX_FILES).contains(&self.files.len())
            && self.files.iter().all(|file| {
                (1..=MAX_LEVELS).contains(&file.location.len())
                    && file.location.iter().all(|part| is_part(part))
                    && is_lower_hex(&file.sha256, 64)
            })
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|file| file.size_bytes).sum()
    }

    /// The `model` member of the result of `copy_adopt` (Section 3): no file name and no path.
    #[must_use]
    pub fn result(&self) -> Value {
        let hashes: Vec<Value> = self
            .files
            .iter()
            .map(|file| json!({ "sizeBytes": file.size_bytes, "sha256": file.sha256 }))
            .collect();
        json!({
            "copy": self.copy,
            "copyState": "downloaded",
            "bytes": self.bytes(),
            "files": self.files.len(),
            "origin": "found",
            "fileHashes": hashes,
        })
    }
}

/// The SHA-256 that a tool names a file by must be the SHA-256 that the daemon read (Section 2.3).
///
/// # Errors
/// `checksum` when the two differ.
pub fn named_hash_holds(model: &FoundModel, read: &[String]) -> Result<(), Cause> {
    let holds = model
        .files
        .iter()
        .zip(read)
        .all(|(file, sha)| file.named_sha256.as_ref().is_none_or(|named| named == sha));
    holds.then_some(()).ok_or(Cause::Checksum)
}
