// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: the words are matched in any case, with no automaton; the levels
// "error" and "fail" are one level; the levels below a warning are not told apart.
// Source: crates/headroom-core/src/transforms/log_compressor.rs (`LevelClassifier`)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
    Other,
}

const ERROR: [&str; 5] = ["error", "fatal", "critical", "fail", "failed"];
const WARNING: [&str; 2] = ["warn", "warning"];

/// The level that the first level word of `line` names. A word is a run of letters, digits and
/// `_`, so `errors` and `error_count` name no level.
#[must_use]
pub fn level(line: &str) -> Level {
    line.split(|each: char| !(each.is_ascii_alphanumeric() || each == '_'))
        .find_map(|word| {
            let word = word.to_ascii_lowercase();
            if ERROR.contains(&word.as_str()) {
                Some(Level::Error)
            } else if WARNING.contains(&word.as_str()) {
                Some(Level::Warning)
            } else {
                None
            }
        })
        .unwrap_or(Level::Other)
}
