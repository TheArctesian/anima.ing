//! The card readings of a device, reduced from the latest sample of each series (D-8).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use daemon_core::readings::Drive;

use super::{Gpu, MetricKind, Readings};
use crate::ids::DeviceId;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Latest {
    pub(super) device: DeviceId,
    pub(super) kind: MetricKind,
    pub(super) label: String,
    pub(super) at: DateTime<Utc>,
    pub(super) value: f64,
    pub(super) gpu: Option<Gpu>,
}

pub(super) const READ_KINDS: [MetricKind; 4] = [
    MetricKind::CpuPercent,
    MetricKind::MemoryPercent,
    MetricKind::DiskPercent,
    MetricKind::VramPercent,
];

/// Disk is the fullest mount; VRAM is the busiest GPU with VRAM, and empty, never 0, without
/// one (MET-007, MET-008, MET-030).
pub(super) fn readings(latest: Vec<Latest>) -> HashMap<DeviceId, Readings> {
    let mut by_device: HashMap<DeviceId, Readings> = HashMap::new();
    for Latest {
        device,
        kind,
        label,
        at,
        value,
        gpu,
    } in latest
    {
        let readings = by_device
            .entry(device)
            .or_insert_with(|| Readings::empty(at));
        readings.at = readings.at.max(at);
        match kind {
            MetricKind::CpuPercent => readings.cpu_percent = Some(value),
            MetricKind::MemoryPercent => readings.memory_percent = Some(value),
            MetricKind::DiskPercent => {
                let fuller = readings.disk_percent.zip(readings.disk_mount.as_deref());
                if fuller.is_none_or(|(top, held)| {
                    value.total_cmp(&top).then(held.cmp(label.as_str())).is_gt()
                }) {
                    readings.disk_percent = Some(value);
                    readings.disk_mount = Some(label);
                }
            }
            MetricKind::VramPercent => {
                // The store also drops a series without VRAM; the filter keeps this rule on its own.
                if let Some(gpu) = gpu.filter(|gpu| gpu.has_vram)
                    && readings
                        .vram_percent
                        .zip(readings.vram_gpu.as_ref())
                        .is_none_or(|(top, held)| {
                            value
                                .total_cmp(&top)
                                .then(held.index.cmp(&gpu.index))
                                .is_gt()
                        })
                {
                    readings.vram_percent = Some(value);
                    readings.vram_gpu = Some(gpu);
                }
            }
            _ => {}
        }
    }
    by_device
}

/// Counts the drives other than the one holding the fullest mount, unmounted ones too (MET-030).
pub(super) fn count_other_drives(
    readings: &mut HashMap<DeviceId, Readings>,
    drives: &HashMap<DeviceId, Vec<Drive>>,
) {
    for (device, readings) in readings.iter_mut() {
        let Some(drives) = drives.get(device) else {
            continue;
        };
        let holds = |drive: &&Drive| {
            readings
                .disk_mount
                .as_ref()
                .is_some_and(|mount| drive.mounts.contains(mount))
        };
        let others = drives.iter().filter(|drive| !holds(drive)).count();
        readings.other_drives = i32::try_from(others).unwrap_or(i32::MAX);
    }
}
