// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: one rule for each language in place of a state machine for each;
// an indented line after an error also counts.
// Source: crates/headroom-core/src/transforms/log_compressor.rs (`is_frame_line`,
// `is_chain_head_line`, `max_stack_trace_lines`)

//! The lines of a stack trace, which stay with the error above them.

/// The most lines of one trace that stay (Headroom: 20).
pub const MOST_LINES: usize = 20;

const HEADS: [&str; 8] = [
    "at ",
    "File \"",
    "Caused by:",
    "Suppressed:",
    "Traceback",
    "During handling",
    "The above exception",
    "... ",
];

/// Whether `line` can be a line of the trace of the error above it: an indented line, a frame
/// or the head of the next cause.
#[must_use]
pub fn continues_trace(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.is_empty()
        && (trimmed.len() < line.len() || HEADS.iter().any(|head| trimmed.starts_with(head)))
}
