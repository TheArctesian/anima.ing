use crate::fleet::metrics::MetricKind;

mod aggregates;
mod capabilities;
mod database;
mod faces;
mod gpu_labels;
mod gpu_reads;
mod gpu_series;
mod kinds;
mod privileges;
mod retention;
mod storage;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{actual} is not {expected}"
    );
}
