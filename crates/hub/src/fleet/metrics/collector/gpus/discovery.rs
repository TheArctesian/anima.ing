//! When the hub looks for a device's GPUs, and how it numbers what it finds (MET-006, MET-007,
//! MET-009). Pure: the time comes in as a parameter.
//!
//! A look is per vendor, so one hung tool never hides the others' GPUs. A failed read counts
//! as no GPU for a vendor with none known (D-30), so a device without that card is not read
//! each round, and as an unfinished read for a vendor with GPUs known, so a broken driver does
//! not hide them until the next 24-hour look.
//!
//! The vendor's own number only matches a reading to its GPU and never becomes the device
//! number, so a hostile one still gives a number from 0 to [`MAX_GPU_INDEX`], the K8 range.
//!
//! The numbers are kept in the collector's memory, not stored: after a hub restart or a pause
//! the first look numbers the GPUs afresh in vendor order. `nvidia-smi` renumbers the GPUs
//! after one drops off the bus and gives no PCI address, so a later NVIDIA GPU can take over
//! the series of the one that dropped.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::{points::MAX_GPU_INDEX, readings::GpuReading};

use super::answers::Answer;
use crate::fleet::metrics::{Gpu, GpuVendor, MAX_LABELS, Reading};

/// How long a discovery holds before the hub looks again (MET-006).
pub(in crate::fleet::metrics::collector) const EVERY: TimeDelta = TimeDelta::hours(24);

/// How much later a device's boot time must read than at the last discovery to count as a
/// restart: one boot reads up to 20 seconds apart (the collection deadline), and a false
/// restart costs only one extra look.
pub(in crate::fleet::metrics::collector) const RESTART_TOLERANCE: TimeDelta =
    TimeDelta::seconds(60);

/// How long after boot a look is provisional: a GPU driver may not be ready yet (a tool that
/// fails reads as no GPU of a vendor with none known), so the hub looks again each round until
/// a look is this far from boot.
pub(in crate::fleet::metrics::collector) const SETTLE: TimeDelta = TimeDelta::minutes(5);

/// A GPU the hub found, with its number in its vendor's own output, which matches each later
/// reading to it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::fleet::metrics::collector) struct Found {
    pub(in crate::fleet::metrics::collector) source: u32,
    pub(in crate::fleet::metrics::collector) gpu: Gpu,
}

/// What the hub knows of a device's GPUs: when it last looked at every vendor, the boot time
/// the device gave then, the GPUs by device number, the vendors whose last read did not
/// finish, and whether a collection failed since.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::fleet::metrics::collector) struct Discovered {
    pub(in crate::fleet::metrics::collector) found_at: DateTime<Utc>,
    pub(in crate::fleet::metrics::collector) boot_at: Option<DateTime<Utc>>,
    pub(in crate::fleet::metrics::collector) gpus: Vec<Found>,
    pub(in crate::fleet::metrics::collector) pending: Vec<GpuVendor>,
    /// A collection failed since the last look: it may have missed a restart or stored a look
    /// without its samples, so the next collection looks at every vendor.
    pub(in crate::fleet::metrics::collector) stale: bool,
}

/// A look a collection makes: at every vendor of the device's system (`only` is `None`), or
/// only at the vendors still pending.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::fleet::metrics::collector) struct Look {
    pub(in crate::fleet::metrics::collector) only: Option<Vec<GpuVendor>>,
    pub(in crate::fleet::metrics::collector) at: DateTime<Utc>,
    pub(in crate::fleet::metrics::collector) boot_at: Option<DateTime<Utc>>,
}

pub(in crate::fleet::metrics::collector) fn boot_at(
    reading: &Reading,
    at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let uptime = Duration::try_from_secs_f64(reading.uptime_seconds?).ok()?;
    at.checked_sub_signed(TimeDelta::from_std(uptime).ok()?)
}

/// The look of the collection at `now`, of a device whose boot time reads `boot_at`, if any.
pub(in crate::fleet::metrics::collector) fn due(
    known: Option<&Discovered>,
    boot_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<Look> {
    let look = |only| Look {
        only,
        at: now,
        boot_at,
    };
    let Some(known) = known else {
        return Some(look(None));
    };
    let expired = now < known.found_at || now - known.found_at >= EVERY;
    let restarted = match (known.boot_at, boot_at) {
        (Some(was), Some(is)) => is - was > RESTART_TOLERANCE,
        _ => false,
    };
    let early = known
        .boot_at
        .is_some_and(|booted| known.found_at - booted < SETTLE);
    if known.stale || expired || restarted || early {
        return Some(look(None));
    }
    (!known.pending.is_empty()).then(|| look(Some(known.pending.clone())))
}

impl Look {
    pub(in crate::fleet::metrics::collector) fn includes(&self, vendor: GpuVendor) -> bool {
        self.only.as_ref().is_none_or(|only| only.contains(&vendor))
    }

    /// What the hub knows after this look, from what it `known` before and the `answers`.
    /// A vendor looked at that answered replaces its GPUs; one whose read did not finish, or
    /// failed with GPUs known (see [`Look::outcome`]), keeps them and is pending. Known GPUs keep their number; new
    /// ones take the lowest free one up to [`MAX_GPU_INDEX`], the first of a repeated own
    /// number, at most [`MAX_LABELS`] in all. Only NVIDIA and AMD GPUs that gave their memory
    /// have VRAM (MET-007).
    pub(in crate::fleet::metrics::collector) fn merge(
        &self,
        known: Option<&Discovered>,
        answers: &[(GpuVendor, Answer)],
    ) -> Discovered {
        let before: &[Found] = known.map_or(&[], |known| &known.gpus);
        let looked: Vec<(GpuVendor, Option<&[GpuReading]>)> = answers
            .iter()
            .filter(|(vendor, _)| self.includes(*vendor))
            .map(|(vendor, answer)| (*vendor, self.outcome(*vendor, answer, known)))
            .collect();
        let answered: Vec<(GpuVendor, &[GpuReading])> = looked
            .iter()
            .filter_map(|(vendor, readings)| readings.map(|readings| (*vendor, readings)))
            .collect();
        let replaced = |vendor: GpuVendor| answered.iter().any(|(each, _)| *each == vendor);
        let mut gpus: Vec<Found> = before
            .iter()
            .filter(|found| !replaced(found.gpu.vendor))
            .cloned()
            .collect();
        let mut new = Vec::new();
        for (vendor, readings) in &answered {
            new.extend(keep_vendor(&mut gpus, before, *vendor, readings));
        }
        for (vendor, reading) in new {
            if gpus.len() >= MAX_LABELS {
                break;
            }
            let free = (0..=i32::from(MAX_GPU_INDEX))
                .find(|free| gpus.iter().all(|found| found.gpu.index != *free));
            gpus.extend(free.map(|index| found(vendor, reading, index)));
        }
        gpus.sort_by_key(|found| found.gpu.index);
        let pending = looked
            .iter()
            .filter(|(_, readings)| readings.is_none())
            .map(|(vendor, _)| *vendor)
            .collect();
        // A look at the pending vendors only does not restart the 24 hours.
        let (found_at, boot_at) = match (known, &self.only) {
            (Some(known), Some(_)) => (known.found_at, known.boot_at),
            _ => (self.at, self.boot_at),
        };
        Discovered {
            found_at,
            boot_at,
            gpus,
            pending,
            stale: false,
        }
    }

    /// The readings `vendor`'s `answer` gives in this look, or `None` when the hub cannot tell
    /// and keeps the GPUs `known` before: a read that did not finish, or a failed read of a
    /// vendor with GPUs known, unless this is a full look and the vendor was already pending.
    /// Otherwise a failed read is no GPU (D-30).
    fn outcome<'a>(
        &self,
        vendor: GpuVendor,
        answer: &'a Answer,
        known: Option<&Discovered>,
    ) -> Option<&'a [GpuReading]> {
        let had = |known: &Discovered| known.gpus.iter().any(|found| found.gpu.vendor == vendor);
        let retry = known.is_some_and(|known| {
            had(known) && (self.only.is_some() || !known.pending.contains(&vendor))
        });
        match answer {
            Answer::Gpus(gpus) => Some(gpus),
            Answer::Failed if retry => None,
            Answer::Failed => Some(&[]),
            Answer::Unknown => None,
        }
    }
}

/// Adds to `gpus` each of `vendor`'s `readings` known `before`, under the number it had, and
/// returns the others, new, the first of a repeated own number, in own-number order. Known
/// numbers are kept first, so a new GPU of an earlier vendor cannot take one.
fn keep_vendor<'a>(
    gpus: &mut Vec<Found>,
    before: &[Found],
    vendor: GpuVendor,
    readings: &'a [GpuReading],
) -> Vec<(GpuVendor, &'a GpuReading)> {
    let mut own: Vec<&GpuReading> = readings.iter().collect();
    own.sort_by_key(|reading| reading.index);
    own.dedup_by_key(|reading| reading.index);
    let mut new = Vec::new();
    for reading in own {
        let was = before
            .iter()
            .find(|found| found.gpu.vendor == vendor && found.source == reading.index);
        match was {
            Some(was) => gpus.push(found(vendor, reading, was.gpu.index)),
            None => new.push((vendor, reading)),
        }
    }
    new
}

fn found(vendor: GpuVendor, reading: &GpuReading, index: i32) -> Found {
    Found {
        source: reading.index,
        gpu: Gpu {
            index,
            name: reading.name.clone(),
            vendor,
            has_vram: reading.vram.is_some()
                && matches!(vendor, GpuVendor::Nvidia | GpuVendor::Amd),
        },
    }
}
