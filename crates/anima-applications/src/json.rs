//! The final form (contract K10 Section 6.4.4, step 5): JSON no second reader reads differently.

use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Json {
    Truth(bool),
    Whole(u64),
    Billionths(u64),
    Text(String),
    List(Vec<Json>),
    Record(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn text(text: &str) -> Self {
        Self::Text(text.to_owned())
    }

    pub(crate) const fn empty() -> Self {
        Self::Record(Vec::new())
    }
}

const fn is_escaped(character: char) -> bool {
    matches!(character, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2028}' | '\u{2029}' | '\u{feff}')
}

/// Each `$` in a value is `$$`, so Compose fills no variable; no surrogate pair is written.
fn string(out: &mut String, text: &str, value: bool) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' if value => out.push_str("$$"),
            escaped if is_escaped(escaped) => {
                // A write to a `String` does not fail.
                let _ = write!(out, "\\u{:04x}", u32::from(escaped));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

fn billionths(out: &mut String, value: u64) {
    let (units, part) = (value / 1_000_000_000, value % 1_000_000_000);
    let part = format!("{part:09}");
    let part = part.trim_end_matches('0');
    let _ = write!(out, "{units}.{}", if part.is_empty() { "0" } else { part });
}

fn write(out: &mut String, json: &Json) {
    match json {
        Json::Truth(truth) => out.push_str(if *truth { "true" } else { "false" }),
        Json::Whole(whole) => {
            let _ = write!(out, "{whole}");
        }
        Json::Billionths(value) => billionths(out, *value),
        Json::Text(text) => string(out, text, true),
        Json::List(values) => {
            out.push('[');
            for (at, value) in values.iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                write(out, value);
            }
            out.push(']');
        }
        Json::Record(members) => {
            out.push('{');
            for (at, (name, value)) in members.iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                string(out, name, false);
                out.push(':');
                write(out, value);
            }
            out.push('}');
        }
    }
}

pub(crate) fn written(json: &Json) -> String {
    let mut out = String::new();
    write(&mut out, json);
    out.push('\n');
    out
}
