//! Forge reads render from JSON, as their documents select different parts of one K1 type.

use serde_json::Value;

use crate::output::{Cell, Tone};

#[must_use]
pub fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("-")
}

#[must_use]
pub fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

#[must_use]
pub fn many<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

#[must_use]
pub fn names(value: &Value, key: &str) -> String {
    let names: Vec<&str> = many(value, key).iter().filter_map(Value::as_str).collect();
    if names.is_empty() {
        "-".to_owned()
    } else {
        names.join(", ")
    }
}

#[must_use]
pub fn number(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map_or_else(|| "-".to_owned(), |number| number.to_string())
}

/// `-` when the checks were not asked for.
#[must_use]
pub fn checks(pull: &Value) -> Cell {
    let Some(checks) = pull.get("checks").filter(|checks| !checks.is_null()) else {
        return "-".into();
    };
    let state = text(checks, "state");
    let tone = match state {
        "PASSED" => Tone::Good,
        "FAILED" => Tone::Bad,
        "PENDING" => Tone::Warning,
        _ => Tone::Muted,
    };
    Cell::toned(state, tone)
}
