use super::{Error, Phase, Record, Result, current, store};
use crate::linux::{builds, workspace};
use daemon_core::artifact::Receipt;
use serde_json::Value;
use uuid::Uuid;

pub(super) async fn release(id: Uuid, confirmed: bool) -> Result<Value> {
    if !confirmed {
        return Err(Error::Refused(
            "Confirm removal of this source job's artifact and workspace.",
        ));
    }
    let record = {
        let _lock = store::lock()?;
        let mut record = current(id)?;
        if record.phase == Phase::Released {
            return Ok(serde_json::to_value(record)?);
        }
        if !matches!(
            record.phase,
            Phase::Ready | Phase::Failed | Phase::Expired | Phase::Cancelled | Phase::Releasing
        ) {
            return Err(Error::Refused(
                "Stop this source job before releasing its storage.",
            ));
        }
        record.phase = Phase::Releasing;
        store::save(&record)?;
        record
    };
    builds::release_if_present(record.build).await?;
    workspace::context::release_if_present(record.workspace, 1, record.context).await?;
    workspace::release_unassigned(record.workspace, 1).await?;
    let _lock = store::lock()?;
    let mut record = current(id)?;
    if !matches!(record.phase, Phase::Releasing | Phase::Released) {
        return Err(Error::Refused("The source-job release intent changed."));
    }
    record.phase = Phase::Released;
    record.result = None;
    store::save(&record)?;
    Ok(serde_json::to_value(record)?)
}

/// The broker records intent. The isolated root worker owns Btrfs and Docker removal.
pub(super) fn queue(
    id: Uuid,
    (device, origin): (Uuid, &str),
    receipt: Option<Receipt>,
) -> Result<Value> {
    let _lock = store::lock()?;
    let mut record = store::read(id)?
        .filter(|record| record.coordinated && record.device == device && record.origin == origin)
        .ok_or(Error::Refused(
            "This execution does not belong to this enrollment.",
        ))?;
    if authorize_release(&mut record, receipt)? {
        store::save(&record)?;
    }
    Ok(serde_json::to_value(record)?)
}

fn authorize_release(record: &mut Record, receipt: Option<Receipt>) -> Result<bool> {
    match record.phase {
        Phase::Releasing | Phase::Released if record.release_receipt == receipt => {
            return Ok(false);
        }
        Phase::Ready
            if receipt
                .as_ref()
                .is_some_and(|receipt| matches_output(record, receipt)) =>
        {
            record.release_receipt = receipt;
        }
        Phase::Failed | Phase::Expired | Phase::Cancelled
            if receipt.is_none() && record.result.is_none() =>
        {
            let cause = match record.phase {
                Phase::Expired => "lease_expired",
                Phase::Cancelled => "operator",
                _ => "step_failed",
            };
            record.failure.get_or_insert_with(|| cause.to_owned());
        }
        _ => {
            return Err(Error::Refused(
                "This source cleanup lacks matching terminal evidence.",
            ));
        }
    }
    record.phase = Phase::Releasing;
    Ok(true)
}

fn matches_output(record: &Record, receipt: &Receipt) -> bool {
    let offer = &receipt.artifact;
    receipt.valid()
        && receipt.device == record.device
        && offer.job == record.job.id
        && offer.attempt == record.job.attempt
        && record.result.as_ref().is_some_and(|result| {
            result["integrityVerified"] == true
                && result["deployed"] == false
                && result["bytes"].as_u64() == Some(offer.bytes)
                && result["archiveDigest"] == offer.archive_digest
                && result["manifestDigest"] == offer.manifest_digest
        })
}

pub(in crate::linux) async fn run() -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused("Source cleanup requires the root owner."));
    }
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o077));
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(10));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        match sweep().await {
            Ok(()) | Err(Error::SourceJournalBusy) => (),
            Err(error) => return Err(error),
        }
    }
}

async fn sweep() -> Result<()> {
    let pending = {
        let _lock = store::lock()?;
        let mut pending = Vec::new();
        for id in store::ids()? {
            if store::read(id)?.is_some_and(|record| record.phase == Phase::Releasing) {
                pending.push(id);
            }
        }
        pending
    };
    for id in pending {
        if release(id, true).await.is_err() {
            eprintln!("Source cleanup awaits runtime reconciliation.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::builds::lease::{Grant, Lease};
    use chrono::{TimeDelta, Utc};
    use daemon_core::artifact::Offer;
    use serde_json::json;

    #[test]
    fn ready_output_requires_its_exact_receipt_to_queue_deletion() {
        let device = Uuid::now_v7();
        let job = Uuid::now_v7();
        let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
        let until = now + TimeDelta::seconds(30);
        let receipt = Receipt {
            version: 1,
            id: Uuid::now_v7(),
            device,
            artifact: Offer {
                job,
                attempt: 1,
                bytes: 42,
                archive_digest: format!("sha256:{}", "a".repeat(64)),
                manifest_digest: format!("sha256:{}", "b".repeat(64)),
            },
        };
        let mut record = Record {
            schema: 1, execution: Uuid::now_v7(), device, origin: "https://example.invalid".into(),
            job: serde_json::from_value(json!({
                "id":job,"type":"build","kind":"container","attempt":1,"lease":{"until":until},
                "limits":{"seconds":60,"idleSeconds":15,"memoryBytes":1_073_741_824_u64,"diskBytes":22_548_578_304_u64,"orphanSeconds":10},
                "project":null,"repository":{"forge":"github","owner":"example","name":"source"},
                "secrets":[],"parameters":{"recipe":"source_image_v1"}
            })).unwrap(),
            lease: Lease::new(Grant { job, attempt: 1, until }).unwrap(),
            registered_at: now, coordinated: true, phase: Phase::Ready,
            workspace: Uuid::now_v7(), context: Uuid::now_v7(), build: Uuid::now_v7(),
            result: Some(json!({
                "integrityVerified":true,"deployed":false,"bytes":42,
                "archiveDigest":receipt.artifact.archive_digest,
                "manifestDigest":receipt.artifact.manifest_digest
            })),
            failure: None, release_receipt: None,
        };
        let mut wrong = receipt.clone();
        wrong.artifact.bytes += 1;
        assert!(authorize_release(&mut record, None).is_err());
        assert!(authorize_release(&mut record, Some(wrong)).is_err());
        assert!(record.phase == Phase::Ready && record.release_receipt.is_none());
        assert!(authorize_release(&mut record, Some(receipt.clone())).unwrap());
        assert!(!authorize_release(&mut record, Some(receipt.clone())).unwrap());
        assert!(record.phase == Phase::Releasing && record.release_receipt == Some(receipt));
    }
}
