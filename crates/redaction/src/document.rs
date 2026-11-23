//! Secrets in a text of many lines (RED-001, COL-005).

use super::{REDACTED, text::redact_text};

/// The texts that all stand in a key block's first line, and the text of its last line.
const KEY_BLOCKS: [(&[&str], &str); 2] = [
    (&["-----BEGIN", "PRIVATE KEY"], "-----END"),
    (&["PuTTY-User-Key-File-"], "Private-MAC:"),
];

/// `text` with each line redacted by [`redact_text`] and each private key block replaced by one
/// line; a block with no last line removes the rest of the text. Line endings stay.
#[must_use]
pub fn redact_document(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut block_end: Option<&str> = None;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let ending = &line[body.len()..];
        if let Some(end) = block_end {
            if body.contains(end) {
                block_end = None;
            }
            continue;
        }
        match block_start(body) {
            Some(end) => {
                redacted.push_str(REDACTED);
                block_end = (!body.contains(end)).then_some(end);
            }
            None => redacted.push_str(&redact_text(body)),
        }
        redacted.push_str(ending);
    }
    redacted
}

fn block_start(line: &str) -> Option<&'static str> {
    KEY_BLOCKS
        .iter()
        .find(|(starts, _)| starts.iter().all(|start| line.contains(start)))
        .map(|(_, end)| *end)
}
