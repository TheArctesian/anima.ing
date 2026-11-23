//! The status a check and a device show: pause, groups and maintenance (CHK-004, CHK-006).
//! Pure: the stored state and the maintenance in force come in as parameters.

use std::collections::HashMap;

use crate::{
    fleet::{devices::Device, maintenance::ActiveTargets},
    ids::{CheckId, DeviceId},
    status::{Status, worst},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Standing {
    pub(super) id: CheckId,
    pub(super) device: DeviceId,
    pub(super) parent: Option<CheckId>,
    pub(super) group: bool,
    pub(super) paused: bool,
    pub(super) status: Status,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cover<'a> {
    pub(super) manual: bool,
    pub(super) active: &'a ActiveTargets,
    pub(super) device: DeviceId,
    pub(super) tags: &'a [String],
}

impl Cover<'_> {
    pub(super) fn covers(&self, check: CheckId) -> bool {
        self.manual || self.active.covers(self.device, Some(check), self.tags)
    }
}

#[must_use]
pub(super) const fn maintained(status: Status, covered: bool) -> Status {
    match status {
        Status::Down if covered => Status::Maintenance,
        other => other,
    }
}

pub(super) fn shown(
    check: &Standing,
    all: &[Standing],
    covered: &dyn Fn(CheckId) -> bool,
) -> Status {
    if check.paused {
        return Status::Unknown;
    }
    if check.group {
        return all
            .iter()
            .filter(|child| child.parent == Some(check.id) && !child.paused)
            .map(|child| shown(child, all, covered))
            .reduce(worst)
            .unwrap_or(Status::Unknown);
    }
    let under = covered(check.id) || check.parent.is_some_and(covered);
    maintained(check.status, under)
}

/// Groups are left out: a group adds nothing beyond its children (CHK-004).
pub(super) fn device_status(checks: &[Standing], covered: &dyn Fn(CheckId) -> bool) -> Status {
    checks
        .iter()
        .filter(|check| !check.group && !check.paused)
        .map(|check| shown(check, checks, covered))
        .reduce(worst)
        .unwrap_or(Status::Unknown)
}

pub(super) fn of_check(
    id: CheckId,
    device: &Device,
    checks: &[Standing],
    active: &ActiveTargets,
) -> Status {
    let cover = Cover {
        manual: device.maintenance,
        active,
        device: device.id,
        tags: &device.tags,
    };
    checks
        .iter()
        .find(|check| check.id == id)
        .map_or(Status::Unknown, |check| {
            shown(check, checks, &|each| cover.covers(each))
        })
}

pub(super) fn by_device(
    requested: &[DeviceId],
    checks: &[Standing],
    known: &[Device],
    active: &ActiveTargets,
) -> HashMap<DeviceId, Status> {
    let devices: HashMap<DeviceId, &Device> = known.iter().map(|d| (d.id, d)).collect();
    let mut grouped: HashMap<DeviceId, Vec<Standing>> = HashMap::new();
    for check in checks {
        grouped.entry(check.device).or_default().push(*check);
    }
    requested
        .iter()
        .map(|&id| {
            let device = devices.get(&id);
            let cover = Cover {
                manual: device.is_some_and(|d| d.maintenance),
                active,
                device: id,
                tags: device.map_or(&[], |d| d.tags.as_slice()),
            };
            let own = grouped.get(&id).map_or(&[][..], Vec::as_slice);
            (id, device_status(own, &|check| cover.covers(check)))
        })
        .collect()
}
