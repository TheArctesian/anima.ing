//! The rules of an `uploaded` report (BLD-063, D-5); publishing checks the feed's content.

use std::collections::HashSet;

use super::model::{FieldError, UploadedFile};

/// The most files one job uploads: the updater file, the DMG and `SHA256SUMS`, with room.
const MAX_FILES: usize = 8;

/// `latest.json` is a few hundred bytes.
const MAX_FEED_BYTES: usize = 64 * 1024;

fn file_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-');
    !bytes.is_empty()
        && bytes.len() <= 128
        && (bytes[0].is_ascii_alphanumeric() || bytes[0] == b'_')
        && bytes.iter().copied().all(allowed)
}

fn sha256(text: &str) -> Option<String> {
    (text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| text.to_ascii_lowercase())
}

pub(super) fn files(reported: &[UploadedFile]) -> Result<Vec<UploadedFile>, FieldError> {
    if reported.is_empty() || reported.len() > MAX_FILES {
        return Err(FieldError::new(
            "files",
            format!("Report 1 to {MAX_FILES} files."),
        ));
    }
    let mut names = HashSet::new();
    reported
        .iter()
        .map(|file| {
            // On a case-insensitive disk or bucket view two such names are one file (T3).
            if !file_name(&file.name) || !names.insert(file.name.to_ascii_lowercase()) {
                return Err(FieldError::new(
                    "files",
                    "Each file needs one plain file name, used once.",
                ));
            }
            let sha256 = sha256(&file.sha256)
                .ok_or_else(|| FieldError::new("files", "Each SHA-256 is 64 hex digits."))?;
            if file.size <= 0 {
                return Err(FieldError::new("files", "Each size is above zero."));
            }
            Ok(UploadedFile {
                name: file.name.clone(),
                sha256,
                size: file.size,
            })
        })
        .collect()
}

/// Stored as the hub writes it out again, so what is stored is exactly what the hub read.
/// Written out it can grow (`1E2` becomes `100.0`), so the limit applies to both.
pub(super) fn feed(text: &str) -> Result<String, FieldError> {
    let refused = || FieldError::new("feed", "The feed is a JSON object of at most 64 KiB.");
    if text.len() > MAX_FEED_BYTES {
        return Err(refused());
    }
    let Ok(object @ serde_json::Value::Object(_)) = serde_json::from_str(text) else {
        return Err(refused());
    };
    serde_json::to_string(&object)
        .ok()
        .filter(|written| written.len() <= MAX_FEED_BYTES)
        .ok_or_else(refused)
}
