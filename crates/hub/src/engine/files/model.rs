//! The records of the files domain and the checks of their text (K-E4 Sections 9 and 10).

use chrono::{DateTime, Utc};

use super::location::Position;
use crate::{
    engine::records::Reference,
    ids::{ArchiveId, AttachmentId},
};

/// The largest file, counted while it streams (FIL-005): 256 MB.
pub const MAX_FILE: u64 = 268_435_456;
const MAX_NAME: usize = 200;
const MAX_MEDIA_TYPE: usize = 100;
pub const NAME_RULE: &str = "Give one line of 1 to 200 characters.";
pub const UNKNOWN_TYPE: &str = "application/octet-stream";

#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub id: AttachmentId,
    pub owner: Reference,
    pub name: String,
    pub media_type: String,
    /// The SHA-256 of the plain bytes, in lower-case hexadecimal.
    pub hash: String,
    pub key_name: String,
    pub size: u64,
    pub archive: Option<ArchiveId>,
    /// Kept only when the operator asked for it at upload (FIL-003).
    pub position: Option<Position>,
    pub created_at: DateTime<Utc>,
}

/// The unit of deletion of one import (FIL-004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archive {
    pub id: ArchiveId,
    pub owner: Reference,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct Removed {
    pub attachments: Vec<Attachment>,
    /// `(key name, hash)` of each piece that no attachment uses now.
    pub pieces: Vec<(String, String)>,
    /// The records each `Derived` deleted, by its name.
    pub derived: Vec<(&'static str, u64)>,
}

#[must_use]
pub fn name(text: &str) -> Option<String> {
    let text = text.trim();
    let fits = !text.is_empty() && text.chars().count() <= MAX_NAME;
    (fits && !text.chars().any(char::is_control)).then(|| text.to_owned())
}

#[must_use]
pub fn media_type(text: Option<&str>) -> Option<String> {
    let Some(text) = text else {
        return Some(UNKNOWN_TYPE.to_owned());
    };
    let essence = text.split(';').next().unwrap_or_default().trim();
    let token = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&b))
    };
    let (kind, subtype) = essence.split_once('/')?;
    (essence.len() <= MAX_MEDIA_TYPE && token(kind) && token(subtype))
        .then(|| essence.to_ascii_lowercase())
}
