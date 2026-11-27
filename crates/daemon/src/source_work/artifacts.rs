use super::{Action, Coordinator, Error, Request, Value};
use daemon_core::{
    artifact::{Offer, Receipt},
    wire::WorkState,
};
use uuid::Uuid;

pub(crate) struct Transfer {
    pub(crate) execution: Uuid,
    pub(crate) offer: Offer,
    pub(crate) request: Request,
}

impl Coordinator {
    pub(crate) fn wants_artifact(&self, execution: Uuid) -> bool {
        self.owned(execution).is_ok_and(|entry| {
            entry.may_run()
                && entry.reported == Some(WorkState::Finishing)
                && entry.artifact_receipt.is_none()
        })
    }

    pub(crate) fn artifact(&self, execution: Uuid, status: &Value) -> Option<Transfer> {
        let entry = self.owned(execution).ok()?;
        let result = &status["result"];
        if !self.wants_artifact(execution)
            || status["phase"] != "ready"
            || result["integrityVerified"] != true
            || result["deployed"] != false
        {
            return None;
        }
        let offer = Offer {
            job: entry.job.id,
            attempt: entry.job.attempt,
            bytes: result["bytes"].as_u64()?,
            archive_digest: result["archiveDigest"].as_str()?.to_owned(),
            manifest_digest: result["manifestDigest"].as_str()?.to_owned(),
        };
        let action = Action::Export {
            execution,
            archive_digest: offer.archive_digest.clone(),
            manifest_digest: offer.manifest_digest.clone(),
        };
        offer.valid().then(|| Transfer {
            execution,
            offer,
            request: Request {
                device: entry.device,
                origin: entry.origin.clone(),
                action,
            },
        })
    }

    pub(crate) fn retain_receipt(
        &mut self,
        transfer: &Transfer,
        receipt: Receipt,
    ) -> Result<(), Error> {
        let mut entry = self.owned(transfer.execution)?.clone();
        if !self.wants_artifact(transfer.execution)
            || !receipt.valid()
            || receipt.device != entry.device
            || receipt.artifact != transfer.offer
        {
            return Err(Error::Conflict);
        }
        entry.artifact_receipt = Some(receipt);
        self.journal.replace(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{home::Home, origin::Origin};
    use serde_json::json;

    #[test]
    fn verified_receipt_is_attempt_bound_and_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::at(temp.path().join("daemon"));
        let device = Uuid::now_v7();
        let origin = Origin::parse("https://example.invalid").unwrap();
        let mut coordinator = Coordinator::open(&home, device, &origin).unwrap();
        let job = super::super::tests::job();
        let mut entry = coordinator
            .journal
            .reserve(device, &origin.as_text(), &job)
            .unwrap();
        let execution = entry.execution;
        let status = json!({"phase":"ready","result":{
            "integrityVerified":true,"deployed":false,"bytes":512,
            "archiveDigest":format!("sha256:{}", "a".repeat(64)),
            "manifestDigest":format!("sha256:{}", "b".repeat(64))
        }});
        assert!(coordinator.artifact(execution, &status).is_none());
        entry.reported = Some(WorkState::Finishing);
        coordinator.journal.replace(entry).unwrap();
        let transfer = coordinator.artifact(execution, &status).unwrap();
        let receipt = Receipt {
            version: 1,
            id: Uuid::now_v7(),
            device,
            artifact: transfer.offer.clone(),
        };
        let mut wrong = receipt.clone();
        wrong.artifact.attempt += 1;
        assert!(coordinator.retain_receipt(&transfer, wrong).is_err());
        assert!(coordinator.wants_artifact(execution));
        coordinator
            .retain_receipt(&transfer, receipt.clone())
            .unwrap();
        drop(coordinator);
        let reopened = Coordinator::open(&home, device, &origin).unwrap();
        assert_eq!(
            reopened.owned(execution).unwrap().artifact_receipt,
            Some(receipt)
        );
        assert!(reopened.artifact(execution, &status).is_none());
    }
}
