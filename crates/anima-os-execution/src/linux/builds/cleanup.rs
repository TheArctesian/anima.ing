use super::{
    super::{
        Error, Result, command,
        workspace::{context, record},
    },
    store,
};
use anima_os_maintenance::{Admission, reservations};
use serde_json::Value;
use std::fs;
use uuid::Uuid;

pub async fn release(id: Uuid, confirmed: bool) -> Result<Value> {
    if !confirmed {
        return Err(Error::Refused(
            "Confirm removal of this build's artifact and scratch storage.",
        ));
    }
    let admission = Admission::cleanup()?;
    let allocation = record::allocation()?;
    let _attempt = store::lock(id)?;
    let mut attempt = store::required(id)?;
    if !matches!(
        attempt.phase.as_str(),
        "cancelled" | "failed" | "verified" | "deleting" | "deleted"
    ) {
        return Err(Error::Refused(
            "Cancel or reconcile this attempt before removing its storage.",
        ));
    }
    finish_release(&admission, &allocation, &mut attempt).await
}

pub(super) async fn finish_release(
    admission: &Admission,
    allocation: &record::Allocation,
    attempt: &mut store::Attempt,
) -> Result<Value> {
    let id = attempt.id;
    reservations::release_build(admission, id, 1).await?;
    context::unpin(allocation, attempt.context, id)?;
    let work = store::base(id).join("work");
    if attempt.phase != "deleted" {
        attempt.phase = "deleting".into();
        store::save(attempt)?;
        match fs::symlink_metadata(&work) {
            Ok(metadata) if metadata.is_dir() => {
                command::run(
                    "btrfs",
                    &[
                        "subvolume",
                        "delete",
                        work.to_str()
                            .ok_or(Error::Refused("Invalid build storage path."))?,
                    ],
                )
                .await?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            _ => return Err(Error::Refused("Build storage needs reconciliation.")),
        }
    }
    match fs::symlink_metadata(&work) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        _ => return Err(Error::Refused("Build storage removal has not completed.")),
    }
    attempt.phase = "deleted".into();
    store::save(attempt)?;
    Ok(serde_json::to_value(attempt)?)
}
