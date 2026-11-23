// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: only the rule "a number more than 2 deviations from the mean" is
// kept; the change points and the kinds of fields of Headroom are not.
// Source: crates/headroom-core/src/transforms/smart_crusher/analyzer.rs and
// crates/headroom-core/src/transforms/smart_crusher/stats_math.rs

//! The rows with a number far from the numbers of the other rows (SHA-001). The measure is
//! the mean and the deviation, as in Headroom: one number that is very large makes the
//! deviation large, and then a second number that is only large is not far. A field whose
//! deviation is not a finite number is not looked at.

use std::collections::BTreeMap;

use serde_json::Value;

/// How many deviations from the mean are far (Headroom: `variance_threshold`).
const FAR: f64 = 2.0;

/// For each row, whether one of its number fields is far from the mean of that field.
#[must_use]
pub fn far_rows(rows: &[Value]) -> Vec<bool> {
    let mut flags = vec![false; rows.len()];
    let mut fields: BTreeMap<&str, Vec<(usize, f64)>> = BTreeMap::new();
    for (row, object) in rows.iter().enumerate() {
        for (name, value) in object.as_object().into_iter().flatten() {
            if let Some(number) = value.as_f64() {
                fields.entry(name.as_str()).or_default().push((row, number));
            }
        }
    }
    for numbers in fields.values() {
        let Some((mean, deviation)) = spread(numbers) else {
            continue;
        };
        for (row, number) in numbers {
            if (number - mean).abs() > FAR * deviation {
                flags[*row] = true;
            }
        }
    }
    flags
}

/// The mean and the sample deviation of the numbers that are there; `None` for fewer than 5
/// numbers or numbers that are all equal.
fn spread(numbers: &[(usize, f64)]) -> Option<(f64, f64)> {
    let present: Vec<f64> = numbers.iter().map(|(_, number)| *number).collect();
    let count = u32::try_from(present.len())
        .ok()
        .filter(|count| *count >= 5)?;
    let mean = present.iter().sum::<f64>() / f64::from(count);
    let squares: f64 = present.iter().map(|number| (number - mean).powi(2)).sum();
    let deviation = (squares / f64::from(count - 1)).sqrt();
    (deviation.is_finite() && deviation > 0.0).then_some((mean, deviation))
}
