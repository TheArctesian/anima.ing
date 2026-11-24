//! Every label value of the exposition passes through [`label_value`], so operator text cannot
//! break a line, close a label or hide characters from the scrape (INT-002).

use crate::text;

/// Longest label value kept, in characters: a check name's limit.
pub(super) const LABEL_MAX: usize = 200;

pub(super) fn label_value(raw: &str) -> String {
    escaped(&text::clean(raw, LABEL_MAX))
}

pub(super) fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}
