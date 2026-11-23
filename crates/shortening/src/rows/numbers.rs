// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: only the rule "a number more than 2 deviations from the mean" is
// kept; the change points and the kinds of fields of Headroom are not.
// Source: crates/headroom-core/src/transforms/smart_crusher/analyzer.rs and
// crates/headroom-core/src/transforms/smart_crusher/stats_math.rs

//! The rows with a number far from the numbers of the other rows (SHA-001).

use std::collections::BTreeSet;

use serde_json::Value;

/// How many deviations from the mean are far (Headroom: `variance_threshold`).
const FAR: f64 = 2.0;

/// For each row, whether one of its number fields is far from the mean of that field.
#[must_use]
pub fn far_rows(rows: &[Value]) -> Vec<bool> {
    let mut flags = vec![false; rows.len()];
    let names: BTreeSet<&str> = rows
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|fields| fields.iter())
        .filter(|(_, value)| value.is_number())
        .map(|(name, _)| name.as_str())
        .collect();
    for name in names {
        let numbers: Vec<Option<f64>> = rows
            .iter()
            .map(|row| row.get(name).and_then(Value::as_f64))
            .collect();
        let Some((mean, deviation)) = spread(&numbers) else {
            continue;
        };
        for (flag, number) in flags.iter_mut().zip(&numbers) {
            if number.is_some_and(|number| (number - mean).abs() > FAR * deviation) {
                *flag = true;
            }
        }
    }
    flags
}

/// The mean and the sample deviation of the numbers that are there; `None` for fewer than 5
/// numbers or numbers that are all equal.
fn spread(numbers: &[Option<f64>]) -> Option<(f64, f64)> {
    let present: Vec<f64> = numbers.iter().flatten().copied().collect();
    let count = u32::try_from(present.len())
        .ok()
        .filter(|count| *count >= 5)?;
    let mean = present.iter().sum::<f64>() / f64::from(count);
    let squares: f64 = present.iter().map(|number| (number - mean).powi(2)).sum();
    let deviation = (squares / f64::from(count - 1)).sqrt();
    (deviation.is_finite() && deviation > 0.0).then_some((mean, deviation))
}
