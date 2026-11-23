// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: one function for rows and lines; the number of rows is fixed, not
// adaptive; a row that must stay is never left out, also above the limit.
// Source: crates/headroom-core/src/transforms/smart_crusher/crushers.rs (`compute_k_split`) and
// crates/headroom-core/src/transforms/smart_crusher/orchestration.rs (`prioritize_indices`,
// `fill_remaining_slots`, `deduplicate_indices_by_content`)

//! Which rows of a list stay (SHA-001, SHA-003, SHA-005).

use std::collections::{BTreeMap, BTreeSet};

use crate::limits::Limits;

/// Why a row stays. The order is the order of the mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// It reports a fault.
    Error,
    /// It has a rare field or a rare value.
    Rare,
    /// It has a number far from the others.
    Far,
    /// It is the first warning of its shape.
    Warning,
    /// It is a line of the stack trace of an error.
    Trace,
    /// It is one of the first rows.
    First,
    /// It is one of the last rows.
    Last,
    /// It is one of the rows taken at an even step from the rest.
    Sample,
}

impl Reason {
    /// The name in the mark of a shortened answer.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Rare => "rare",
            Self::Far => "far",
            Self::Warning => "warning",
            Self::Trace => "trace",
            Self::First => "first",
            Self::Last => "last",
            Self::Sample => "sample",
        }
    }
}

/// What is known of each row before the choice.
pub struct Marked {
    /// For each row, why it must stay, if it must.
    pub must: Vec<Option<Reason>>,
    /// For each row, a text that is equal for rows that say the same.
    pub same: Vec<String>,
}

/// The rows that stay, in their order, each with its reason.
pub type Kept = BTreeMap<usize, Reason>;

/// The rows of a list that stay; `None` when the list is short or no row is left out.
#[must_use]
pub fn choice(rows: &Marked, limits: &Limits) -> Option<Kept> {
    let total = rows.must.len();
    if total < limits.smallest_list || total <= limits.kept_rows {
        return None;
    }
    let mut kept: Kept = rows
        .must
        .iter()
        .enumerate()
        .filter_map(|(row, reason)| reason.map(|reason| (row, reason)))
        .collect();
    for row in 0..limits.first_rows.min(total) {
        kept.entry(row).or_insert(Reason::First);
    }
    for row in total.saturating_sub(limits.last_rows)..total {
        kept.entry(row).or_insert(Reason::Last);
    }
    let free = limits.kept_rows.saturating_sub(kept.len());
    for row in sample(&new_rows(rows, &kept), free) {
        kept.insert(row, Reason::Sample);
    }
    (kept.len() < total).then_some(kept)
}

/// The rows that are not kept and that say something no row before them said.
fn new_rows(rows: &Marked, kept: &Kept) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    let mut new = Vec::new();
    for (row, same) in rows.same.iter().enumerate() {
        if seen.insert(same.as_str()) && !kept.contains_key(&row) {
            new.push(row);
        }
    }
    new
}

/// `count` of `rows` at an even step.
fn sample(rows: &[usize], count: usize) -> Vec<usize> {
    let count = count.min(rows.len());
    (0..count)
        .map(|step| rows[step * rows.len() / count])
        .collect()
}
