//! A link between two records (K-E4; RLK-001, RLK-002).

use chrono::{DateTime, Utc};

use crate::{engine::records::Reference, ids::RecordLinkId};

const MAX_KIND: usize = 40;
const MAX_LABEL: usize = 200;

/// What a link means, such as `lent_to`; the extension that makes the link names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation(String);

impl Relation {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut chars = text.chars();
        let valid = chars.next().is_some_and(|first| first.is_ascii_lowercase())
            && text.len() <= MAX_KIND
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        valid.then(|| Self(text.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("a label is one line of at most 200 characters")]
pub struct BadLabel;

/// The text a link shows while its target has no record (RLK-002); blank text is no label.
///
/// # Errors
/// [`BadLabel`] for text with a control character or more than 200 characters.
pub fn label(text: &str) -> Result<Option<String>, BadLabel> {
    let text = text.trim();
    if text.chars().any(char::is_control) || text.chars().count() > MAX_LABEL {
        Err(BadLabel)
    } else {
        Ok(Some(text.to_owned()).filter(|label| !label.is_empty()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLink {
    pub source: Reference,
    pub target: Reference,
    pub relation: Relation,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub id: RecordLinkId,
    pub source: Reference,
    pub target: Reference,
    pub relation: Relation,
    /// Kept for as long as the link exists, also after the target has a record.
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Made,
    Relabelled,
    /// The link existed as asked; nothing was written.
    Kept,
}
