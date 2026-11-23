//! Whether the hub takes a parsed batch (K8 Sections 4 and 6, MET-017). Pure: the device and the
//! clock are parameters (CQ-021); the hub transport refuses the bearer, size and JSON first.

use std::collections::BTreeSet;

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use super::batteries::batteries_fit;
use super::{
    Batch, BatchBattery, BatchDrive, BatchGpu, BatchPoint, BatchSeries, BatchUpdate, GpuVendor,
    KeyVersion, RefusalCode, versions_fit,
};
use crate::agent_usage::AgentUsage;
use crate::hardware::Hardware;
use crate::inventory::Inventory;
use crate::points::{
    Kind, MAX_GPU_INDEX, MAX_LABELS, MAX_POINTS, MAX_SERIES, Point, SeriesKey, Summary, ceiling,
    is_label, is_window_end,
};

const OLDEST: TimeDelta = TimeDelta::hours(1);

const AHEAD: TimeDelta = TimeDelta::seconds(60);

const MAX_VERSION_BYTES: usize = 64;

/// What the hub takes from a batch (K8 Section 4); the inventory holds only the tools this hub
/// knows by name.
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    pub points: Vec<Point>,
    pub gpus: Option<Vec<BatchGpu>>,
    pub drives: Option<Vec<BatchDrive>>,
    pub batteries: Option<Vec<BatchBattery>>,
    pub inventory: Option<Inventory>,
    /// With only the backends this hub knows (CAP-032).
    pub hardware: Option<Hardware>,
    pub daemon_version: Option<String>,
    /// With only the session days of known sources (R-135).
    pub agent_usage: Option<AgentUsage>,
    pub keys: Option<Vec<KeyVersion>>,
    pub update: Option<BatchUpdate>,
}

impl Batch {
    /// # Errors
    /// The first refusal of K8 Section 6, in the order `WrongDevice`, `Malformed`, `OutOfRange`.
    /// A series of a kind this hub does not know is held to the counts and "no two equal" only.
    pub fn check(self, device: Uuid, now: DateTime<Utc>) -> Result<Accepted, RefusalCode> {
        if self.device != device {
            return Err(RefusalCode::WrongDevice);
        }
        let keys = series_keys(&self.series).ok_or(RefusalCode::Malformed)?;
        let shaped = points_fit(&self.points, &keys)
            && gpus_fit(self.gpus.as_deref())
            && drives_fit(self.drives.as_deref())
            && batteries_fit(self.batteries.as_deref())
            && self.daemon_version.as_deref().is_none_or(is_version)
            && self
                .keys
                .as_ref()
                .is_none_or(|keys| versions_fit(&keys.held))
            && self
                .update
                .as_ref()
                .is_none_or(|update| update.fits(is_version));
        if !shaped {
            return Err(RefusalCode::Malformed);
        }
        let inventory = match self.inventory {
            Some(sent) => Some(sent.accepted().ok_or(RefusalCode::Malformed)?),
            None => None,
        };
        let hardware = match self.hardware {
            Some(sent) => Some(sent.accepted().ok_or(RefusalCode::Malformed)?),
            None => None,
        };
        let agent_usage = match self.agent_usage {
            Some(sent) => Some(sent.accepted().ok_or(RefusalCode::Malformed)?),
            None => None,
        };
        if !in_range(&self.points, now) {
            return Err(RefusalCode::OutOfRange);
        }
        let points = self
            .points
            .into_iter()
            .map(|point| accepted(point, &keys))
            .collect();
        Ok(Accepted {
            points,
            gpus: self.gpus,
            drives: self.drives,
            batteries: self.batteries,
            inventory,
            hardware,
            daemon_version: self.daemon_version,
            agent_usage,
            keys: self.keys.map(|keys| keys.held),
            update: self.update,
        })
    }
}

fn series_keys(series: &[BatchSeries]) -> Option<Vec<Option<SeriesKey>>> {
    if !(1..=MAX_SERIES).contains(&series.len()) {
        return None;
    }
    let distinct: BTreeSet<(&str, Option<&str>, Option<u8>)> = series
        .iter()
        .map(|one| (one.kind.as_str(), one.label.as_deref(), one.gpu))
        .collect();
    if distinct.len() != series.len() {
        return None;
    }
    series
        .iter()
        .map(|one| match Kind::parse(&one.kind) {
            Some(kind) => SeriesKey::new(kind, one.label.clone(), one.gpu).map(Some),
            None => Some(None),
        })
        .collect()
}

fn points_fit(points: &[BatchPoint], keys: &[Option<SeriesKey>]) -> bool {
    let increasing = points.windows(2).all(|pair| pair[0].at < pair[1].at);
    (1..=MAX_POINTS).contains(&points.len())
        && increasing
        && points.iter().all(|point| point_fits(point, keys))
}

fn point_fits(point: &BatchPoint, keys: &[Option<SeriesKey>]) -> bool {
    let columns = keys.len();
    if !is_window_end(point.at) || point.mean.len() != columns {
        return false;
    }
    let max = match &point.max {
        Some(max) if max.len() == columns => max.as_slice(),
        Some(_) => return false,
        None => &[],
    };
    keys.iter().enumerate().all(|(column, key)| {
        let Some(key) = key else { return true };
        let max = max.get(column).copied().flatten();
        value_fits(key.kind(), point.mean[column], max)
    })
}

fn value_fits(kind: Kind, mean: Option<f64>, max: Option<f64>) -> bool {
    match (mean, max) {
        (_, Some(_)) if !kind.has_max() => false,
        (None, max) => max.is_none(),
        (Some(mean), None) => number_fits(kind, mean),
        (Some(mean), Some(max)) => number_fits(kind, mean) && number_fits(kind, max) && max >= mean,
    }
}

/// `-0` is refused, as a sample refuses it.
fn number_fits(kind: Kind, value: f64) -> bool {
    value.is_finite()
        && !value.is_sign_negative()
        && (!kind.is_percent() || value <= 100.0)
        && value <= ceiling(kind)
}

fn gpus_fit(gpus: Option<&[BatchGpu]>) -> bool {
    let gpus = gpus.unwrap_or_default();
    let indexes: BTreeSet<u8> = gpus.iter().map(|gpu| gpu.index).collect();
    indexes.len() == gpus.len()
        && gpus.iter().all(|gpu| {
            let shares_memory = matches!(gpu.vendor, GpuVendor::Intel | GpuVendor::Apple);
            gpu.index <= MAX_GPU_INDEX && is_label(&gpu.name) && !(shares_memory && gpu.has_vram)
        })
}

fn drives_fit(drives: Option<&[BatchDrive]>) -> bool {
    let drives = drives.unwrap_or_default();
    let names: BTreeSet<&str> = drives.iter().map(|drive| drive.name.as_str()).collect();
    names.len() == drives.len()
        && drives.len() <= MAX_LABELS
        && drives.iter().all(|drive| {
            is_label(&drive.name)
                && i64::try_from(drive.size_bytes).is_ok()
                && drive.mounts.len() <= MAX_LABELS
                && drive
                    .mounts
                    .iter()
                    .all(|mount| mount.starts_with('/') && is_label(mount))
        })
}

/// A Semantic Versioning version fits, and no control character or space does (R-122).
fn is_version(text: &str) -> bool {
    text.len() <= MAX_VERSION_BYTES
        && text
            .bytes()
            .next()
            .is_some_and(|first| first.is_ascii_digit())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
}

/// The points are in order, so the two ends decide.
fn in_range(points: &[BatchPoint], now: DateTime<Utc>) -> bool {
    let oldest_fits = points.first().is_some_and(|point| now - point.at <= OLDEST);
    let newest_fits = points.last().is_some_and(|point| point.at - now <= AHEAD);
    oldest_fits && newest_fits
}

fn accepted(point: BatchPoint, keys: &[Option<SeriesKey>]) -> Point {
    let max = point.max.unwrap_or_default();
    let values = keys
        .iter()
        .zip(point.mean)
        .enumerate()
        .filter_map(|(column, (key, mean))| {
            let key = key.as_ref()?;
            let summary = Summary {
                mean: mean?,
                max: max.get(column).copied().flatten(),
            };
            Some((key.clone(), summary))
        })
        .collect();
    Point::new(point.at, values)
}
