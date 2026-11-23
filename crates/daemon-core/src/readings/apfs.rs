//! The drives of a macOS device, read from its `df -P -k` volumes (MET-027, MET-031).
//! Each APFS volume reports its whole container as its total, so each container becomes one
//! drive and only one of its volumes stays a disk.

use std::collections::BTreeMap;

use super::{Disk, Drive, DriveKind, label};
use crate::points::{MAX_LABELS, is_operator_mount};

const DEVICE_PREFIX: &str = "/dev/disk";
const DATA_VOLUME: &str = "/System/Volumes/Data";

/// The container of an APFS volume's device: `disk3` for `/dev/disk3s5` or `/dev/disk3s1s1`.
fn container(filesystem: &str) -> Option<&str> {
    let number = filesystem.strip_prefix(DEVICE_PREFIX)?;
    let digits = number.bytes().take_while(u8::is_ascii_digit).count();
    let rest = number.get(digits..)?;
    if digits == 0 || !rest.starts_with('s') {
        return None;
    }
    filesystem.get("/dev/".len()..DEVICE_PREFIX.len() + digits)
}

fn chosen(volumes: &[&Disk]) -> Option<usize> {
    volumes
        .iter()
        .position(|volume| volume.mount == DATA_VOLUME)
        .or_else(|| {
            (0..volumes.len())
                .filter(|&at| is_operator_mount(&volumes[at].mount))
                .min_by(|&a, &b| volumes[a].mount.cmp(&volumes[b].mount))
        })
}

/// The disks that keep their own series, and the drives of the APFS containers; `None` when no
/// disk is a volume of a container.
#[must_use]
pub fn containers(disks: Vec<Disk>) -> (Vec<Disk>, Option<Vec<Drive>>) {
    let mut grouped: BTreeMap<String, Vec<&Disk>> = BTreeMap::new();
    for disk in &disks {
        if let Some(name) = container(&disk.filesystem).and_then(label) {
            grouped.entry(name.to_owned()).or_default().push(disk);
        }
    }
    if grouped.is_empty() {
        return (disks, None);
    }
    let mut drives = Vec::new();
    let mut kept_mounts = Vec::new();
    for (name, volumes) in &grouped {
        let Some(at) = chosen(volumes) else { continue };
        let volume = volumes[at];
        if volume.total_bytes == 0 || i64::try_from(volume.total_bytes).is_err() {
            continue;
        }
        kept_mounts.push(volume.mount.clone());
        drives.push(Drive {
            name: name.clone(),
            kind: DriveKind::Disk,
            size_bytes: volume.total_bytes,
            mounts: vec![volume.mount.clone()],
        });
    }
    drives.truncate(MAX_LABELS);
    let disks = disks
        .into_iter()
        .filter(|disk| container(&disk.filesystem).is_none() || kept_mounts.contains(&disk.mount))
        .collect();
    (disks, Some(drives))
}
