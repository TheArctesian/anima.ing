//! A builder's log lines as stored (BLD-048, BLD-049, D-4): redacted, cleaned and bounded.

use super::{hidden_text, model::FieldError, private_keys};
use crate::fleet::events;

pub(super) const MAX_LINES_PER_REPORT: usize = 1000;

const MAX_LINE_BYTES: usize = 4096;

/// The store drops the oldest lines past this (BLD-048).
pub(super) const MAX_JOB_LOG_BYTES: i64 = 2 * 1024 * 1024;

/// Per-line overhead, so many short lines cannot keep an unbounded number of rows.
pub(super) const LINE_OVERHEAD_BYTES: i64 = 64;

const MAX_REASON_CHARS: usize = 200;

const REDACTED: &str = "[redacted]";

pub(super) const KEY_BLOCK: &str = "[redacted: private key block]";

/// A key ending in this (any ASCII case) holds a signature, such as a presigned URL's (T3).
const SIGNATURE: &str = "signature";

/// PostgreSQL text refuses NUL, and a stored line holds no line break.
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
        .collect()
}

fn cut(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

fn value_end(text: &str, start: usize) -> usize {
    let end = text[start..]
        .find(|c: char| c.is_whitespace() || matches!(c, '&' | ';' | ',' | '}' | '"' | '\''))
        .map_or(text.len(), |offset| start + offset);
    text[start..end]
        .find("%26")
        .map_or(end, |offset| start + offset)
}

fn after_separator(text: &str) -> Option<&str> {
    text.strip_prefix(['=', ':']).or_else(|| {
        let encoded = text.get(..3)?;
        (encoded.eq_ignore_ascii_case("%3d") || encoded.eq_ignore_ascii_case("%3a"))
            .then(|| &text[3..])
    })
}

fn mask_signatures(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut masked = String::with_capacity(text.len());
    let (mut copied, mut index) = (0, 0);
    while let Some(offset) = lower[index..].find(SIGNATURE) {
        index += offset + SIGNATURE.len();
        let rest = &text[index..];
        let after_key = rest.trim_start_matches(['"', '\'']).trim_start();
        let Some(value) = after_separator(after_key) else {
            continue;
        };
        let start = text.len() - value.trim_start_matches([' ', '"', '\'']).len();
        let end = value_end(text, start);
        if end > start {
            masked.push_str(&text[copied..start]);
            masked.push_str(REDACTED);
            (copied, index) = (end, end);
        }
    }
    masked.push_str(&text[copied..]);
    masked
}

/// A minisign public key's id is kept: the build log may show the public key.
fn redact(text: &str) -> String {
    if private_keys::is_secret_text(text) {
        return REDACTED.to_owned();
    }
    if let Some(colon) = private_keys::public_key_id_colon(text) {
        return format!("{}:{}", redact(&text[..colon]), redact(&text[colon + 1..]));
    }
    mask_signatures(&events::redact_text(text))
}

/// `None` when the masked values cannot be told apart from the text around them.
fn masked_values<'a>(text: &'a str, masked: &str) -> Option<Vec<&'a str>> {
    let mut pieces = masked.split(REDACTED);
    let first = pieces.next().unwrap_or_default();
    let mut rest = text.strip_prefix(first)?;
    let mut values = Vec::new();
    let mut pieces = pieces.peekable();
    while let Some(piece) = pieces.next() {
        let end = if pieces.peek().is_none() {
            rest.strip_suffix(piece)?.len()
        } else if piece.is_empty() {
            return None;
        } else {
            rest.find(piece)?
        };
        values.push(&rest[..end]);
        rest = &rest[end + piece.len()..];
    }
    Some(values)
}

fn leaks(reading: &str, stored: &str) -> bool {
    let masked = redact(reading);
    if masked == reading {
        return false;
    }
    if masked == REDACTED {
        return stored != REDACTED;
    }
    masked_values(reading, &masked).is_none_or(|values| {
        values
            .iter()
            .any(|value| !value.is_empty() && stored.contains(value))
    })
}

/// A line as a terminal shows it, and three stricter readings that keep what an escape may take.
struct Readings {
    shown: String,
    stricter: [String; 3],
}

impl Readings {
    fn of(line: &str) -> Self {
        Self {
            shown: printable(&hidden_text::visible(line)),
            stricter: [
                printable(&hidden_text::spaced(line)),
                printable(&hidden_text::joined(line)),
                printable(&hidden_text::joined_rare(line)),
            ],
        }
    }

    fn all(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.shown.as_str()).chain(self.stricter.iter().map(String::as_str))
    }

    /// Removing an escape can take a key's first letter (`ESC [ p` then `assword=…`), so a
    /// value a stricter reading masks redacts the whole line (BLD-049, LOG-007).
    fn redacted(&self) -> String {
        let shown = redact(&self.shown);
        let leaked = self
            .stricter
            .iter()
            .any(|reading| *reading != self.shown && leaks(reading, &shown));
        if leaked { REDACTED.to_owned() } else { shown }
    }

    fn opens_key(&self) -> Option<u16> {
        self.all().filter_map(private_keys::opens).max()
    }

    fn closes_key(&self) -> bool {
        self.all().any(private_keys::closes)
    }

    fn is_key_material(&self) -> bool {
        self.all().any(private_keys::is_material)
    }
}

/// Redaction comes before the cut, so the cut never leaves part of a secret (BLD-049).
pub(super) fn clean(line: &str) -> String {
    cut(Readings::of(line).redacted(), MAX_LINE_BYTES)
}

/// A key block may run across reports; `open` is how many more lines it may take. Its lines
/// after the opening one are dropped while they are key material; the first other line ends it.
pub(super) fn clean_all(
    lines: &[String],
    mut open: Option<u16>,
) -> Result<(Vec<String>, Option<u16>), FieldError> {
    if lines.len() > MAX_LINES_PER_REPORT {
        return Err(FieldError::new(
            "logLines",
            format!("Send at most {MAX_LINES_PER_REPORT} lines in one report."),
        ));
    }
    let mut kept = Vec::with_capacity(lines.len());
    for line in lines {
        let readings = Readings::of(line);
        if let Some(left) = open.take() {
            if readings.closes_key() {
                continue;
            }
            if left > 0 && readings.is_key_material() {
                open = Some(left - 1);
                continue;
            }
        }
        if let Some(block) = readings.opens_key() {
            open = (!readings.closes_key()).then_some(block);
            kept.push(KEY_BLOCK.to_owned());
            continue;
        }
        let cleaned = cut(readings.redacted(), MAX_LINE_BYTES);
        if !cleaned.trim().is_empty() {
            kept.push(cleaned);
        }
    }
    Ok((kept, open))
}

pub(super) fn reason(text: Option<&str>) -> String {
    let cleaned = clean(text.unwrap_or_default());
    let trimmed: String = cleaned.trim().chars().take(MAX_REASON_CHARS).collect();
    if trimmed.is_empty() {
        "the builder reported a failure".to_owned()
    } else {
        trimmed
    }
}
