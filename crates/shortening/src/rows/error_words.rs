// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: Headroom looks for a word in the whole JSON text of a row, the
// names of the fields included. Anima looks in each text value, and in the name of a field only
// when the field holds something, so that a field `lastError: null` in each row keeps no row.
// Source: crates/headroom-core/src/transforms/smart_crusher/error_keywords.rs and
// crates/headroom-core/src/transforms/smart_crusher/outliers.rs

//! The words that say that a row or a line reports a fault (SHA-001).

use serde_json::Value;

/// The 12 words of Headroom, in lower case. A word inside a longer word counts: it is better
/// to keep one row too many.
pub const ERROR_WORDS: [&str; 12] = [
    "error",
    "exception",
    "failed",
    "failure",
    "critical",
    "fatal",
    "crash",
    "panic",
    "abort",
    "timeout",
    "denied",
    "rejected",
];

/// Whether `text` holds an error word, with case ignored.
#[must_use]
pub fn names_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    ERROR_WORDS.iter().any(|word| lower.contains(word))
}

/// Whether `row` reports a fault: a text value with an error word at any depth, or a field
/// whose name has an error word and that holds something.
#[must_use]
pub fn reports_error(row: &Value) -> bool {
    match row {
        Value::String(text) => names_error(text),
        Value::Array(values) => values.iter().any(reports_error),
        Value::Object(fields) => fields
            .iter()
            .any(|(name, value)| reports_error(value) || (names_error(name) && holds(value))),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// Whether a field says something: not null, not false, not zero, not empty.
fn holds(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(set) => *set,
        Value::Number(number) => number.as_f64().is_none_or(|number| number != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}
