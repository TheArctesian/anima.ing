use super::{Error, Phase, Result, store, work_policy};
use crate::linux::{command, workspace};
use anima_os_maintenance::{Admission, reservations};
use serde_json::{Value, json};

pub(super) async fn observe() -> Result<Value> {
    for service in [
        "docker.service",
        "anima-build-leases.service",
        "anima-source-cleanup.service",
    ] {
        command::run("systemctl", &["is-active", "--quiet", service]).await?;
    }
    let admission = Admission::acquire()?;
    let allocation = workspace::record::allocation()?;
    let _journal = store::lock()?;
    let policy = work_policy::Request::WorkPolicy.execute()?;
    let enabled = policy["enabled"] == true;
    let repositories = policy["repositories"]
        .as_str()
        .ok_or(Error::Refused("Source repository policy is unavailable."))?;
    let ids = store::ids()?;
    let mut occupied = false;
    for id in &ids {
        let record = store::read(*id)?.ok_or(Error::Refused("Source inventory changed."))?;
        occupied |= record.phase.executing()
            || record.phase == Phase::Releasing
            || (record.phase == Phase::Registered && !record.lease.expired());
    }
    let room = workspace::has_room()? && ids.len() < 128;
    let resources = reservations::available(&admission)?;
    let disk = workspace::record::free_storage(&allocation)?.min(24 * 1024_u64.pow(3));
    let memory = resources.memory_bytes.min(2 * 1024_u64.pow(3));
    let available = enabled
        && !repositories.trim().is_empty()
        && room
        && !occupied
        && resources.cpu_millis >= 500
        && resources.tasks >= 128
        && memory >= 512 * 1024 * 1024
        && disk >= 21 * 1024_u64.pow(3);
    Ok(
        json!({"enabled":enabled,"available":available,"repositories":repositories,"privateCheckout":true,"free":{
            "slots":u32::from(available),
            "memoryBytes":if available {memory} else {0},
            "diskBytes":if available {disk} else {0}
        }}),
    )
}
