//! Sample the same limits that workspace creation and launch enforce.
use agent_threads::managed::workspace::Capacity;
use anima_os_maintenance::{Admission, reservations};

use super::{Result, workspace};

pub(super) fn observe() -> Result<Capacity> {
    let admission = Admission::acquire()?;
    let resources = reservations::available(&admission)?;
    let allocation = workspace::record::allocation()?;
    let maximum = Capacity::MAXIMUM;
    let storage_bytes = if workspace::has_room()? {
        workspace::record::free_storage(&allocation)?.min(maximum.storage_bytes)
    } else {
        0
    };
    Ok(Capacity {
        storage_bytes,
        memory_bytes: resources.memory_bytes.min(maximum.memory_bytes),
        cpu_millis: resources.cpu_millis.min(maximum.cpu_millis),
        tasks: resources.tasks.min(maximum.tasks),
    })
}
