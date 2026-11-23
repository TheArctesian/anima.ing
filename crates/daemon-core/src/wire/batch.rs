//! The push body: the points of one device, column by column (K8 Section 4).

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{BatchAgentUsage, BatchBattery, BatchInventory, BatchKeys, BatchUpdate, KeyVersion};
use crate::hardware::Hardware;
use crate::points::{Point, SeriesKey};
use crate::readings::{self, Drive};

/// `POST /daemon/v1/push` (K8 Section 4); [`Batch::check`] says whether the hub takes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    pub device: Uuid,
    pub series: Vec<BatchSeries>,
    pub points: Vec<BatchPoint>,
    /// Sent after start and after each discovery; absent means unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<Vec<BatchGpu>>,
    /// Sent after start and when they change; absent means unchanged (MET-027).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drives: Option<Vec<BatchDrive>>,
    /// Sent after start and when one comes, goes or changes its state; absent means unchanged
    /// (MET-032).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batteries: Option<Vec<BatchBattery>>,
    /// Sent after start and when a probe result changes; absent means unchanged (CAP-027, CAP-029).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<BatchInventory>,
    /// Sent with the inventory; absent means unchanged (CAP-032, R-165).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware: Option<Hardware>,
    #[serde(
        rename = "daemonVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub daemon_version: Option<String>,
    /// The daemon's state and the session days whose counts changed; absent means nothing new
    /// (R-135, AIS-019).
    #[serde(
        rename = "agentUsage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_usage: Option<BatchAgentUsage>,
    /// The versions in the daemon's key cache (K-E5 Section 2.1, R-147); a daemon without a key
    /// cache sends none and is told no keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<BatchKeys>,
    /// A daemon that cannot update itself sends none (K8 Section 8.1, R-139, ROL-044).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<BatchUpdate>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Announcements {
    pub gpus: Option<Vec<BatchGpu>>,
    pub drives: Option<Vec<BatchDrive>>,
    pub batteries: Option<Vec<BatchBattery>>,
    pub inventory: Option<BatchInventory>,
    pub hardware: Option<Hardware>,
    pub agent_usage: Option<BatchAgentUsage>,
}

/// `kind` is text, so a kind this hub does not know can be ignored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchSeries {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<u8>,
}

/// The window ending at `at`; `null` for a series with no sample in the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatchPoint {
    #[serde(with = "super::times")]
    pub at: DateTime<Utc>,
    pub mean: Vec<Option<f64>>,
    /// Present when some column has a maximum; non-null only for CPU and GPU use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<Option<f64>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchGpu {
    pub index: u8,
    pub name: String,
    pub vendor: GpuVendor,
    /// False for Intel and Apple, whose GPUs share system memory (MET-007).
    pub has_vram: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDrive {
    pub name: String,
    pub kind: DriveKind,
    pub size_bytes: u64,
    pub mounts: Vec<String>,
}

/// A kind a newer daemon knows and this hub does not reads as `disk`, so the batch is not refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriveKind {
    Nvme,
    Ssd,
    Hdd,
    #[serde(other)]
    Disk,
}

impl From<&Drive> for BatchDrive {
    fn from(drive: &Drive) -> Self {
        Self {
            name: drive.name.clone(),
            kind: match drive.kind {
                readings::DriveKind::Nvme => DriveKind::Nvme,
                readings::DriveKind::Ssd => DriveKind::Ssd,
                readings::DriveKind::Hdd => DriveKind::Hdd,
                readings::DriveKind::Disk => DriveKind::Disk,
            },
            size_bytes: drive.size_bytes,
            mounts: drive.mounts.clone(),
        }
    }
}

impl From<BatchDrive> for Drive {
    fn from(drive: BatchDrive) -> Self {
        Self {
            name: drive.name,
            kind: match drive.kind {
                DriveKind::Nvme => readings::DriveKind::Nvme,
                DriveKind::Ssd => readings::DriveKind::Ssd,
                DriveKind::Hdd => readings::DriveKind::Hdd,
                DriveKind::Disk => readings::DriveKind::Disk,
            },
            size_bytes: drive.size_bytes,
            mounts: drive.mounts,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
}

impl Batch {
    #[must_use]
    pub fn new<'a>(
        device: Uuid,
        points: impl IntoIterator<Item = &'a Point>,
        announced: Announcements,
    ) -> Self {
        let points: Vec<&Point> = points.into_iter().collect();
        let keys: BTreeSet<&SeriesKey> = points
            .iter()
            .flat_map(|point| point.values().keys())
            .collect();
        Self {
            device,
            series: keys.iter().map(|key| BatchSeries::of(key)).collect(),
            points: points
                .iter()
                .map(|point| BatchPoint::of(point, &keys))
                .collect(),
            gpus: announced.gpus,
            drives: announced.drives,
            batteries: announced.batteries,
            inventory: announced.inventory,
            hardware: announced.hardware,
            daemon_version: None,
            agent_usage: announced.agent_usage,
            keys: None,
            update: None,
        }
    }

    #[must_use]
    pub fn with_keys(self, held: Vec<KeyVersion>) -> Self {
        Self {
            keys: Some(BatchKeys { held }),
            ..self
        }
    }

    #[must_use]
    pub fn with_update(self, update: BatchUpdate) -> Self {
        Self {
            update: Some(update),
            ..self
        }
    }

    #[must_use]
    pub fn from_daemon(self, version: &str) -> Self {
        Self {
            daemon_version: Some(version.to_owned()),
            ..self
        }
    }
}

impl BatchSeries {
    fn of(key: &SeriesKey) -> Self {
        Self {
            kind: key.kind().as_str().to_owned(),
            label: key.label().map(str::to_owned),
            gpu: key.gpu(),
        }
    }
}

impl BatchPoint {
    fn of(point: &Point, keys: &BTreeSet<&SeriesKey>) -> Self {
        let values = point.values();
        let mean = keys
            .iter()
            .map(|key| values.get(*key).map(|summary| summary.mean))
            .collect();
        let max: Vec<Option<f64>> = keys
            .iter()
            .map(|key| values.get(*key).and_then(|summary| summary.max))
            .collect();
        Self {
            at: point.at(),
            mean,
            max: max.iter().any(Option::is_some).then_some(max),
        }
    }
}
