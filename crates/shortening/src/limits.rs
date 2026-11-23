// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: only the numbers that Anima uses are kept; the two fractions are
// whole rows; the gates of the answer are new.
// Source: crates/headroom-core/src/transforms/smart_crusher/config.rs

//! The numbers of the shortener (SHA-007).

/// When a list is shortened and how many rows stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// An answer below this many bytes of JSON is not touched.
    pub smallest_answer: usize,
    /// A list below this many rows is not touched (Headroom: `min_items_to_analyze`).
    pub smallest_list: usize,
    /// The rows that stay, before the rows that must stay (Headroom: `max_items_after_crush`).
    pub kept_rows: usize,
    /// The rows that stay from the start (Headroom: `first_fraction` 0.3 of 15).
    pub first_rows: usize,
    /// The rows that stay from the end (Headroom: `last_fraction` 0.15 of 15).
    pub last_rows: usize,
    /// A change that saves less than this part of the answer, in percent, is not made.
    pub least_saving_percent: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            smallest_answer: 2_000,
            smallest_list: 5,
            kept_rows: 15,
            first_rows: 4,
            last_rows: 2,
            least_saving_percent: 15,
        }
    }
}
