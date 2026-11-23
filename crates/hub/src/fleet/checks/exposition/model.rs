//! One exposition's values, read once and rendered without further reads (INT-001).

use crate::{
    fleet::checks::{UptimeWindow, config::CheckType},
    ids::{CheckId, DeviceId, SiteId},
    status::Status,
};

/// The fixed `window` label values of `anima_check_uptime_ratio` (D-24); [`CheckSample::uptime`] follows this order.
pub(super) const WINDOWS: [(UptimeWindow, &str); 3] = [
    (UptimeWindow::Day, "1d"),
    (UptimeWindow::Week, "7d"),
    (UptimeWindow::Month, "30d"),
];

/// Only values that rarely change (INT-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DeviceLabels {
    pub(super) id: DeviceId,
    pub(super) name: String,
    pub(super) site: SiteId,
}

/// One device and the status it shows (CHK-004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DeviceSample {
    pub(super) device: DeviceLabels,
    pub(super) status: Status,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CheckSample {
    pub(super) id: CheckId,
    pub(super) name: String,
    pub(super) kind: CheckType,
    pub(super) device: DeviceLabels,
    /// Pause, groups and maintenance applied, as `Check.status`.
    pub(super) status: Status,
    pub(super) response_time_ms: Option<u32>,
    pub(super) uptime: [Option<f64>; 3],
    /// As `Check.certificateDaysLeft`.
    pub(super) certificate_days_left: Option<i32>,
    pub(super) domain_days_left: Option<i32>,
}

/// In the order the exposition lists them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Snapshot {
    pub(super) checks: Vec<CheckSample>,
    pub(super) devices: Vec<DeviceSample>,
}
