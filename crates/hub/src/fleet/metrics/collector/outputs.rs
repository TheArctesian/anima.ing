//! The reading, distribution and samples that one collection's outputs give (MET-002, DEV-016, D-4).

use std::time::Duration;

use chrono::{DateTime, Utc};
use daemon_core::readings::{
    Drive, apfs, df, drives, linux,
    macos::{self, Sysctl},
};

use super::os_release;
use crate::fleet::{
    devices::Distribution,
    metrics::{Memory, Reading, Sample, commands::Source, samples},
    ssh::CommandOutput,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Previous {
    pub(super) at: DateTime<Utc>,
    pub(super) reading: Reading,
}

pub(super) fn usable(output: &CommandOutput) -> Option<&[u8]> {
    let finished = !output.cut && output.exit_status == Some(0);
    finished.then_some(output.stdout.as_slice())
}

/// Built one output at a time, so each output's bytes drop before the next command runs (D-11).
#[derive(Debug, Default)]
pub(super) struct Parts {
    reading: Reading,
    sysctl: Option<Sysctl>,
    used_bytes: Option<u64>,
    distribution: Option<Distribution>,
}

impl Parts {
    /// `vm_stat` may need the page size from `sysctl`, which the command table runs first.
    pub(super) fn add(&mut self, source: Source, output: &CommandOutput) {
        let Some(bytes) = usable(output) else {
            return;
        };
        let reading = &mut self.reading;
        match source {
            Source::Stat => reading.cpu = linux::cpu(bytes),
            Source::Top => reading.cpu = macos::cpu(bytes),
            Source::Meminfo => reading.memory = linux::memory(bytes),
            Source::NetDev => reading.interfaces = linux::interfaces(bytes),
            Source::Netstat => reading.interfaces = macos::interfaces(bytes),
            Source::Uptime => reading.uptime_seconds = linux::uptime_seconds(bytes),
            Source::Loadavg => reading.load = linux::load_average(bytes),
            Source::DiskBytes => reading.disks = df::disks(bytes, 1),
            Source::DiskKilobytes => {
                (reading.disks, reading.drives) = apfs::containers(df::disks(bytes, 1024));
            }
            Source::Diskstats => reading.drive_counters = linux::drive_counters(bytes),
            Source::Drives | Source::DrivesOld => reading.drives = drives::drives(bytes),
            Source::Sysctl => self.sysctl = Some(macos::sysctl(bytes)),
            Source::VmStat => {
                let page_size = self.sysctl.and_then(|sysctl| sysctl.page_size);
                self.used_bytes = macos::memory_used_bytes(bytes, page_size);
            }
            Source::OsRelease => self.distribution = os_release::distribution(bytes),
        }
    }

    /// The older `lsblk` form runs only when the newer one gave no drives (MET-027).
    pub(super) fn needs(&self, source: Source) -> bool {
        source != Source::DrivesOld || self.reading.drives.is_none()
    }

    pub(super) fn take_distribution(&mut self) -> Option<Distribution> {
        self.distribution.take()
    }

    pub(super) fn reading(self, at: DateTime<Utc>) -> Reading {
        let Self {
            mut reading,
            sysctl,
            used_bytes,
            distribution: _,
        } = self;
        let Some(sysctl) = sysctl else {
            return reading;
        };
        reading.load = reading.load.or(sysctl.load);
        reading.uptime_seconds = reading.uptime_seconds.or_else(|| sysctl.uptime_seconds(at));
        if let (None, Some(total_bytes), Some(used_bytes)) =
            (reading.memory, sysctl.memory_bytes, used_bytes)
        {
            reading.memory = Some(Memory {
                total_bytes,
                used_bytes,
            });
        }
        reading
    }
}

/// A previous reading at or after `at` (the hub clock went back) gives no rates (D-4).
pub(super) fn collected(
    previous: Option<&Previous>,
    reading: &Reading,
    at: DateTime<Utc>,
) -> Vec<Sample> {
    let elapsed = previous
        .and_then(|previous| (at - previous.at).to_std().ok())
        .unwrap_or(Duration::ZERO);
    samples(previous.map(|previous| &previous.reading), reading, elapsed)
}

/// Only a list that differs from the previous one is stored; an unread list keeps the stored one (MET-027).
pub(super) fn new_drives<'a>(
    previous: Option<&Previous>,
    reading: &'a Reading,
) -> Option<&'a [Drive]> {
    let read = reading.drives.as_deref()?;
    let before = previous.and_then(|previous| previous.reading.drives.as_deref());
    (before != Some(read)).then_some(read)
}
