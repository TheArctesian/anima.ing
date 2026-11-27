//! A local operator can pause new work while existing owners finish or stop it.
use crate::{Admission, Error, ROOT, read_small_receipt, update_window, write_receipt};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    time::SystemTime,
};
use uuid::Uuid;

const RECEIPT: &str = "/run/anima-os/work-drain.json";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Drain {
    schema: u32,
    id: Uuid,
}

/// # Errors
/// A power operation has ownership, or the drain record could not be read or saved.
pub fn begin() -> Result<Drain, Error> {
    let _admission = Admission::configuration()?;
    if let Some(drain) = status()? {
        return Ok(drain);
    }
    let drain = Drain {
        schema: 1,
        id: Uuid::now_v7(),
    };
    write_receipt(RECEIPT, "work-drain.pending", &drain)?;
    Ok(drain)
}

/// # Errors
/// A power operation has ownership, the identity is stale, or storage failed.
pub fn cancel(id: Uuid) -> Result<(), Error> {
    let _admission = Admission::configuration()?;
    if status()?.is_none_or(|drain| drain.id != id) {
        return Err(Error::Refused(
            "The work drain changed. Review maintenance again.",
        ));
    }
    fs::remove_file(RECEIPT)?;
    File::open(ROOT)?.sync_all()?;
    Ok(())
}

pub(super) fn require_open(opened_at: SystemTime) -> Result<(), Error> {
    if status()?.is_some() {
        return Err(Error::Refused(
            "This node is draining work for maintenance. Queue the work or choose another node.",
        ));
    }
    if update_window::active_until(opened_at)?.is_some() {
        return Err(Error::Refused(
            "The update activation window is holding new work. Queue the work or choose another node.",
        ));
    }
    Ok(())
}

pub(super) fn status() -> Result<Option<Drain>, Error> {
    let drain: Option<Drain> = read_small_receipt(RECEIPT)?;
    if drain
        .as_ref()
        .is_some_and(|drain| drain.schema != 1 || drain.id.is_nil())
    {
        return Err(Error::Refused("The work drain record is invalid."));
    }
    Ok(drain)
}
