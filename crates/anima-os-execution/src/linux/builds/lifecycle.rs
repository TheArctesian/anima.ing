use super::{
    super::{
        Error, Result, command,
        workspace::{context, record},
    },
    cleanup,
    output::progress,
    store, verification,
};
use anima_os_maintenance::{Admission, reservations};
use serde_json::{Value, json};
use uuid::Uuid;

pub async fn status(id: Uuid) -> Result<Value> {
    let _attempt = store::lock(id)?;
    let attempt = store::required(id)?;
    let runtime = command::run(
        "systemctl",
        &[
            "show",
            &store::unit(id),
            "--property=ActiveState,SubState,MainPID,Result",
        ],
    )
    .await;
    let observed = runtime.ok().map(|text| {
        text.lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.to_owned(), Value::String(value.to_owned())))
            .collect::<serde_json::Map<_, _>>()
    });
    let progress = progress::observe(&store::base(id).join("work"));
    let progress_available = progress.is_ok();
    Ok(
        json!({"attempt":attempt,"runtime":observed,"workerProgress":progress.ok().flatten(),
        "workerProgressAvailable":progress_available}),
    )
}

pub async fn cancel(id: Uuid) -> Result<Value> {
    let admission = Admission::cleanup()?;
    let allocation = record::allocation()?;
    let _attempt = store::lock(id)?;
    let mut attempt = store::required(id)?;
    if attempt.phase == "deleting" {
        return cleanup::finish_release(&admission, &allocation, &mut attempt).await;
    }
    if matches!(
        attempt.phase.as_str(),
        "cancelled" | "failed" | "verified" | "deleted"
    ) {
        return Ok(serde_json::to_value(attempt)?);
    }
    attempt.phase = "cancelling".into();
    store::save(&attempt)?;
    finish_cancellation(&admission, &allocation, &mut attempt).await
}

async fn finish_cancellation(
    admission: &Admission,
    allocation: &record::Allocation,
    attempt: &mut store::Attempt,
) -> Result<Value> {
    let id = attempt.id;
    // Even when stop reports a missing unit, the independent termination proof
    // below must succeed before releasing either resources or the source lease.
    let _ = command::run("systemctl", &["stop", &store::unit(id)]).await;
    reservations::release_build(admission, id, 1).await?;
    context::unpin(allocation, attempt.context, id)?;
    attempt.phase = "cancelled".into();
    store::save(attempt)?;
    Ok(serde_json::to_value(attempt)?)
}

pub async fn reconcile(id: Uuid) -> Result<Value> {
    match begin_reconciliation(id).await? {
        Reconciliation::Finished(value) => Ok(value),
        Reconciliation::Pending(pending) => pending.complete().await,
    }
}

enum Reconciliation {
    Finished(Value),
    Pending(verification::Pending),
}

async fn begin_reconciliation(id: Uuid) -> Result<Reconciliation> {
    let admission = Admission::cleanup()?;
    let allocation = record::allocation()?;
    let _attempt = store::lock(id)?;
    let mut attempt = store::required(id)?;
    if super::lease::expired_attempt(&attempt) {
        attempt.phase = "cancelling".into();
        store::save(&attempt)?;
    }
    if attempt.phase == "cancelling" {
        return finish_cancellation(&admission, &allocation, &mut attempt)
            .await
            .map(Reconciliation::Finished);
    }
    if attempt.phase == "deleting" {
        return cleanup::finish_release(&admission, &allocation, &mut attempt)
            .await
            .map(Reconciliation::Finished);
    }
    if matches!(
        attempt.phase.as_str(),
        "cancelled" | "failed" | "verified" | "deleted"
    ) {
        return Ok(Reconciliation::Finished(serde_json::to_value(attempt)?));
    }
    reservations::release_build(&admission, id, 1).await?;
    let result = store::base(id).join("work/result.json");
    if result.try_exists()? {
        let result: Value = store::read(&result)?;
        if result["schema"] != 1 {
            return Err(Error::Refused("The build result needs reconciliation."));
        }
        if result["phase"] == "exported" {
            let pending = verification::Pending::seal(id, result).await?;
            context::unpin(&allocation, attempt.context, id)?;
            return Ok(Reconciliation::Pending(pending));
        }
    }
    attempt.phase = "failed".into();
    context::unpin(&allocation, attempt.context, id)?;
    store::save(&attempt)?;
    Ok(Reconciliation::Finished(serde_json::to_value(attempt)?))
}

pub(super) async fn expire(id: Uuid) -> Result<()> {
    let admission = Admission::cleanup()?;
    let allocation = record::allocation()?;
    let _attempt = store::lock(id)?;
    let mut attempt = store::required(id)?;
    if attempt.phase == "cancelling" {
        finish_cancellation(&admission, &allocation, &mut attempt).await?;
    } else if super::lease::expired_attempt(&attempt) {
        attempt.phase = "cancelling".into();
        store::save(&attempt)?;
        finish_cancellation(&admission, &allocation, &mut attempt).await?;
    }
    Ok(())
}
