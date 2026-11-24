//! An accepted batch's rows, labelled by the SSH collector's rule (MET-015, K8 Section 4, R-099).

use std::collections::BTreeMap;

use daemon_core::points::{Point, SeriesKey};

use crate::fleet::metrics::{Sample, store::PointValue};

pub(super) struct Rows {
    pub(super) series: Vec<Sample>,
    pub(super) values: Vec<PointValue>,
}

/// A value no sample keeps is left out, as a collection leaves it out.
pub(super) fn rows(points: &[Point]) -> Rows {
    let mut series: BTreeMap<&SeriesKey, Sample> = BTreeMap::new();
    let mut values = Vec::new();
    for point in points {
        for (key, summary) in point.values() {
            let Some(sample) = sample_of(key, summary.mean) else {
                continue;
            };
            series.entry(key).or_insert_with(|| sample.clone());
            values.push(PointValue {
                at: point.at(),
                sample,
                max: summary.max,
            });
        }
    }
    Rows {
        series: series.into_values().collect(),
        values,
    }
}

fn sample_of(key: &SeriesKey, mean: f64) -> Option<Sample> {
    match key.gpu() {
        Some(index) => Sample::gpu(key.kind(), index, mean),
        None => Sample::new(key.kind(), key.label().unwrap_or_default(), mean),
    }
}
