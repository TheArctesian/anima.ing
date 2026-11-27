//! Measure only exact mounts from the installed volume record.
use std::{os::fd::OwnedFd, path::Path};

use rustix::fs::{self, AtFlags, Mode, OFlags, StatxFlags};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, PERSIST, RECORD_NAME, Result, VOLUME_MOUNTS, Volume, command, records};

#[derive(Serialize)]
pub struct VolumeObservation {
    uuid: Uuid,
    model: Option<String>,
    mountpoint: String,
    #[serde(flatten)]
    observation: Observation,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum Observation {
    NotMounted,
    UnexpectedMount,
    Unavailable,
    Mounted { capacity: Capacity },
}

#[derive(Serialize)]
struct Capacity {
    total: u64,
    used: u64,
    available: u64,
}

#[derive(Deserialize)]
struct MountTable {
    filesystems: Vec<Mount>,
}

#[derive(Deserialize)]
struct Mount {
    target: String,
    id: u64,
    fstype: String,
    uuid: Option<String>,
}

/// Read the installer-recorded volumes and sample capacity for verified mounts.
///
/// # Errors
/// Requires root and a readable volume record and mount table.
pub fn inventory() -> Result<Vec<VolumeObservation>> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Refused(
            "Volume inventory requires local root authority.",
        ));
    }
    let volumes = records::read(&Path::new(PERSIST).join(RECORD_NAME))?.unwrap_or_default();
    if volumes.is_empty() {
        return Ok(Vec::new());
    }
    // Hold mount instances before reading the table so their IDs cannot be recycled.
    let observed: Vec<_> = volumes
        .into_iter()
        .map(|volume| {
            let mountpoint = format!("{VOLUME_MOUNTS}/{}", volume.uuid);
            let held = open_mount(&mountpoint);
            (volume, mountpoint, held)
        })
        .collect();
    let output = command::capture(
        "findmnt",
        &[
            "--json",
            "--list",
            "--kernel",
            "--output",
            "TARGET,ID,FSTYPE,UUID",
        ],
        "4s",
    )?;
    let table: MountTable = serde_json::from_slice(&output)?;
    Ok(observed
        .into_iter()
        .map(|(volume, mountpoint, held)| project(volume, mountpoint, held, &table.filesystems))
        .collect())
}

fn project(
    volume: Volume,
    mountpoint: String,
    held: Option<(OwnedFd, u64)>,
    mounts: &[Mount],
) -> VolumeObservation {
    let observation = mounted(&mountpoint, &volume.uuid.to_string(), held, mounts);
    VolumeObservation {
        uuid: volume.uuid,
        model: volume.model,
        mountpoint,
        observation,
    }
}

fn mounted(path: &str, uuid: &str, held: Option<(OwnedFd, u64)>, mounts: &[Mount]) -> Observation {
    let mut at_path = mounts
        .iter()
        .filter(|mount| mount.target == path)
        .peekable();
    if at_path.peek().is_none() {
        return Observation::NotMounted;
    }
    let Some((fd, id)) = held else {
        return Observation::Unavailable;
    };
    let Some(mount) = at_path.find(|mount| mount.id == id) else {
        return Observation::Unavailable;
    };
    if mount.fstype != "btrfs" || mount.uuid.as_ref().is_some_and(|observed| observed != uuid) {
        return Observation::UnexpectedMount;
    }
    if mount.uuid.is_none() {
        return Observation::Unavailable;
    }
    let Ok(facts) = fs::fstatvfs(fd) else {
        return Observation::Unavailable;
    };
    let bytes = |blocks: u64| blocks.saturating_mul(facts.f_frsize);
    Observation::Mounted {
        capacity: Capacity {
            total: bytes(facts.f_blocks),
            used: bytes(facts.f_blocks.saturating_sub(facts.f_bfree)),
            available: bytes(facts.f_bavail),
        },
    }
}

fn open_mount(path: &str) -> Option<(OwnedFd, u64)> {
    let fd = fs::open(
        path,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    let stat = fs::statx(&fd, "", AtFlags::EMPTY_PATH, StatxFlags::MNT_ID).ok()?;
    // Read capacity from the same mount instance even if its path is reassigned.
    (stat.stx_mask & StatxFlags::MNT_ID.bits() != 0).then_some((fd, stat.stx_mnt_id))
}
