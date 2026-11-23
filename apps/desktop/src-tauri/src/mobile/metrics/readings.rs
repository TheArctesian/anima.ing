use std::path::Path;

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
use ios::{cpu_percent, memory};

use chrono::{DateTime, Utc};
use daemon_core::{
    points::Kind,
    wire::{Batch, BatchPoint, BatchSeries},
};
use uuid::Uuid;

#[cfg(target_os = "android")]
fn memory() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kib = |key| {
        let line = text.lines().find_map(|line| line.strip_prefix(key))?;
        line.split_whitespace().next()?.parse::<u64>().ok()
    };
    let total = kib("MemTotal:")?.checked_mul(1024)?;
    let available = kib("MemAvailable:")?.checked_mul(1024)?;
    Some((total, total.checked_sub(available)?))
}

#[cfg(target_os = "android")]
fn storage(path: &Path) -> Option<(u64, u64)> {
    let stats = rustix::fs::statvfs(path).ok()?;
    let total = stats.f_blocks.checked_mul(stats.f_frsize)?;
    let used = stats
        .f_blocks
        .checked_sub(stats.f_bfree)?
        .checked_mul(stats.f_frsize)?;
    Some((total, used))
}

#[derive(Default)]
struct Samples {
    series: Vec<BatchSeries>,
    mean: Vec<Option<f64>>,
}

impl Samples {
    fn push(&mut self, kind: Kind, label: Option<&str>, value: f64) {
        self.series.push(BatchSeries {
            kind: kind.as_str().to_owned(),
            label: label.map(str::to_owned),
            gpu: None,
        });
        self.mean.push(Some(value));
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "the metrics wire carries byte counts as floats"
    )]
    fn add_usage(&mut self, kinds: (Kind, Kind), label: Option<&str>, (total, used): (u64, u64)) {
        if total > 0 {
            self.push(kinds.0, label, used as f64);
            self.push(kinds.1, label, 100.0 * used as f64 / total as f64);
        }
    }
}

pub(super) fn batch(device: Uuid, now: DateTime<Utc>, path: &Path) -> Option<Batch> {
    let mut samples = Samples::default();
    #[cfg(target_os = "ios")]
    {
        let _ = path;
        if let Some(percent) = cpu_percent() {
            samples.push(Kind::CpuPercent, None, percent);
        }
    }
    #[cfg(target_os = "android")]
    if let Some(reading) = storage(path) {
        samples.add_usage((Kind::DiskBytes, Kind::DiskPercent), Some("/data"), reading);
    }
    if let Some(reading) = memory() {
        samples.add_usage((Kind::MemoryBytes, Kind::MemoryPercent), None, reading);
    }
    if samples.series.is_empty() {
        return None;
    }
    let seconds = now.timestamp();
    let at = DateTime::from_timestamp(seconds - seconds.rem_euclid(10), 0)?;
    Some(Batch {
        device,
        series: samples.series,
        points: vec![BatchPoint {
            at,
            mean: samples.mean,
            max: None,
        }],
        gpus: None,
        drives: None,
        batteries: None,
        inventory: None,
        hardware: None,
        daemon_version: None,
        agent_usage: None,
        agent_threads: None,
        keys: None,
        update: None,
    })
}
