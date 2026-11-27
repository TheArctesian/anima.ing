use super::{Error, Phase, Result, checkout, current, snapshot, store, supervisor};
use crate::linux::{builds, command, workspace};
use anima_os_maintenance::{Admission, source_pipeline};
use serde_json::Value;
use uuid::Uuid;

pub(super) async fn cancel(id: Uuid) -> Result<Value> {
    let executing = {
        let _lock = store::lock()?;
        let mut record = current(id)?;
        if record.phase == Phase::Registered {
            record.phase = Phase::Cancelled;
            store::save(&record)?;
        } else if record.phase.executing() {
            record.phase = Phase::Cancelling;
            record.failure.get_or_insert_with(|| "operator".into());
            record.result = None;
            store::save(&record)?;
        }
        record.phase.executing()
    };
    if executing {
        stop(id).await?;
    }
    Ok(serde_json::to_value(snapshot(id)?)?)
}

pub(super) async fn stop(id: Uuid) -> Result<()> {
    let _ = command::run("systemctl", &["stop", &supervisor::unit(id)]).await;
    finish(id).await
}

pub(in crate::linux) async fn finish(id: Uuid) -> Result<()> {
    let pid = command::run(
        "systemctl",
        &[
            "show",
            &supervisor::unit(id),
            "--property=MainPID",
            "--value",
        ],
    )
    .await?;
    if pid.trim() != "0" {
        return Err(Error::Refused("The source supervisor has not stopped."));
    }
    checkout::remove(id)?;
    let record = snapshot(id)?;
    if !record.phase.executing() {
        return Ok(());
    }
    builds::cancel_if_present(record.build).await?;
    workspace::stop_if_present(record.workspace, 1).await?;
    let admission = Admission::cleanup()?;
    let _lock = store::lock()?;
    let mut record = current(id)?;
    if !record.phase.executing() {
        return Ok(());
    }
    if record.phase == Phase::Finishing && record.result.is_some() {
        record.phase = Phase::Ready;
    } else {
        record.phase = match record.failure.as_deref() {
            Some("operator") => Phase::Cancelled,
            Some("lease_expired") => Phase::Expired,
            _ => Phase::Failed,
        };
        record.result = None;
        record
            .failure
            .get_or_insert_with(|| "execution_stopped".into());
    }
    store::save(&record)?;
    source_pipeline::retire(&admission, id)?;
    Ok(())
}
