// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: the two rules are cut into small functions, the fractions are
// whole-number arithmetic, and the result is one flag for each row. A value is rare only when
// fewer than 10 % of the rows have it: with Headroom's rule alone, one of five values that are
// equally frequent counts as rare.
// Source: crates/headroom-core/src/transforms/smart_crusher/outliers.rs
// (`detect_structural_outliers`, `detect_rare_status_values`)

//! The rows that are not like the others: a rare field, or a rare value of a common field
//! (SHA-001).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// A field is common when at least 80 % of the rows have it.
const COMMON_PERCENT: usize = 80;
/// A field is rare when fewer than 20 % of the rows have it.
const RARE_PERCENT: usize = 20;
/// A field with more different values than this is not a state (a name, an identifier).
const MOST_VALUES: usize = 50;
/// The values that cover 80 % of the rows are the usual ones, when they are at most this many.
const MOST_USUAL: usize = 5;
/// A value that is not usual is rare when fewer than 10 % of the rows have it.
const RARE_VALUE_PERCENT: usize = 10;

/// For each row, whether it has a rare field or a rare value.
#[must_use]
pub fn rare_rows(rows: &[Value]) -> Vec<bool> {
    let counts = field_counts(rows);
    let total = rows.len();
    let rare: BTreeSet<&str> = counts
        .iter()
        .filter(|(_, count)| **count * 100 < total * RARE_PERCENT)
        .map(|(name, _)| *name)
        .collect();
    let mut flags: Vec<bool> = rows
        .iter()
        .map(|row| {
            row.as_object()
                .is_some_and(|fields| fields.keys().any(|name| rare.contains(name.as_str())))
        })
        .collect();
    let common = counts
        .iter()
        .filter(|(_, count)| **count * 100 >= total * COMMON_PERCENT);
    for (name, _) in common {
        mark_rare_values(rows, name, &mut flags);
    }
    flags
}

fn field_counts(rows: &[Value]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for fields in rows.iter().filter_map(Value::as_object) {
        for name in fields.keys() {
            *counts.entry(name.as_str()).or_insert(0) += 1;
        }
    }
    counts
}

/// The value of a field as one text, so that values of different kinds compare.
fn shown(value: &Value) -> String {
    match value {
        Value::Null => "\u{0}null".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Flags the rows whose value of `name` is not one of the usual values.
fn mark_rare_values(rows: &[Value], name: &str, flags: &mut [bool]) {
    let values: Vec<Option<String>> = rows
        .iter()
        .map(|row| {
            row.as_object()
                .and_then(|fields| fields.get(name))
                .map(shown)
        })
        .collect();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for value in values.iter().flatten() {
        *counts.entry(value.as_str()).or_insert(0) += 1;
    }
    if !(2..=MOST_VALUES).contains(&counts.len()) {
        return;
    }
    let Some(usual) = usual_values(&counts) else {
        return;
    };
    let total: usize = counts.values().sum();
    let rare =
        |value: &str| !usual.contains(value) && counts[value] * 100 < total * RARE_VALUE_PERCENT;
    for (flag, value) in flags.iter_mut().zip(&values) {
        if value.as_deref().is_some_and(rare) {
            *flag = true;
        }
    }
}

/// The most frequent values that together cover 80 % of the rows that have the field; `None`
/// when they are more than [`MOST_USUAL`], so the field is not a state.
fn usual_values<'a>(counts: &BTreeMap<&'a str, usize>) -> Option<BTreeSet<&'a str>> {
    let total: usize = counts.values().sum();
    let mut ordered: Vec<(&str, usize)> = counts
        .iter()
        .map(|(value, count)| (*value, *count))
        .collect();
    ordered.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    let mut usual = BTreeSet::new();
    let mut covered = 0;
    for (value, count) in ordered {
        covered += count;
        usual.insert(value);
        if covered * 100 >= total * COMMON_PERCENT {
            break;
        }
    }
    (usual.len() <= MOST_USUAL).then_some(usual)
}
