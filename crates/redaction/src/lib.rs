//! The one redaction of secrets in text (RED-001, LOG-007).
//!
//! It prefers over-matching to letting a secret through.
//! Redact before the text is sealed or clipped: a clipped secret no longer has its shape (RED-003).
//! A new shape needs a new line in `fixtures/secret-shapes.txt` (RED-004).

mod document;
mod names;
mod provider_keys;
#[cfg(test)]
mod tests;
mod text;

pub use document::redact_document;
pub use names::{Secrecy, normalise, secrecy};
pub use text::redact_text;

pub const REDACTED: &str = "[redacted]";
