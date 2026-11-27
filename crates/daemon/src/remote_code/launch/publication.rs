//! Durable coordination between the remote runner and the normal K8 metrics publisher.

use std::collections::BTreeMap;

use daemon_core::wire::{
    ThreadSummaries, ThreadSummariesReceipt, ThreadSummary, ThreadSummaryReceipt,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DispatchRequest, Origin, Store, StoreError, find};

pub(crate) type CredentialStamp = [u8; 32];

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Intent {
    revision: i64,
    credential: CredentialStamp,
    thread: Option<Uuid>,
    accepted: bool,
    complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationProgress {
    Waiting,
    Accepted,
    Complete,
}

impl Store {
    pub(crate) fn request_publication(
        &self,
        request: &DispatchRequest,
        credential: CredentialStamp,
        revision: impl FnOnce() -> Option<i64>,
    ) -> Result<PublicationProgress, StoreError> {
        let mut locked = self.lock()?;
        let saved = find(&mut locked.journal, request)?;
        if !saved.publication_open() {
            return Err(StoreError::Conflict);
        }
        let previous = saved.publication.as_ref();
        if previous.is_some_and(|intent| intent.complete) {
            return Ok(PublicationProgress::Complete);
        }
        if let Some(intent) = previous
            && intent.credential == credential
        {
            return Ok(if intent.accepted {
                PublicationProgress::Accepted
            } else {
                PublicationProgress::Waiting
            });
        }
        saved.publication = Some(Intent {
            revision: revision().ok_or(StoreError::Busy)?,
            credential,
            thread: previous.and_then(|intent| intent.thread),
            accepted: false,
            complete: false,
        });
        locked.save()?;
        Ok(PublicationProgress::Waiting)
    }

    pub(crate) fn publication_requests(
        &self,
        device: Uuid,
        hub: &Origin,
    ) -> Result<BTreeMap<Uuid, i64>, StoreError> {
        let locked = self.lock()?;
        Ok(locked
            .journal
            .launches
            .values()
            .filter_map(|saved| {
                let intent = saved.publication.as_ref()?;
                (saved.enrolled_at(device, hub) && saved.publication_open() && !intent.accepted)
                    .then_some((saved.request.runner, intent.revision))
            })
            .collect())
    }

    pub(crate) fn acknowledge_publication(
        &self,
        (device, hub, credential): (Uuid, &Origin, CredentialStamp),
        sent: &ThreadSummaries,
        receipt: &ThreadSummariesReceipt,
    ) -> Result<(), StoreError> {
        let mut locked = self.lock()?;
        let mut changed = false;
        for saved in locked
            .journal
            .launches
            .values_mut()
            .filter(|saved| saved.enrolled_at(device, hub) && saved.publication_open())
        {
            let Some(intent) = saved
                .publication
                .as_mut()
                .filter(|intent| !intent.accepted && intent.credential == credential)
            else {
                continue;
            };
            let Some(sent) = sent.summaries.iter().find(|sent| {
                sent.runner_id == saved.request.runner && sent.revision >= intent.revision
            }) else {
                continue;
            };
            if let Some(accepted) = matching_summary(sent, receipt)
                && intent.thread.is_none_or(|id| accepted.id == id)
            {
                intent.thread = Some(accepted.id);
                intent.accepted = true;
                changed = true;
            }
        }
        if changed {
            locked.save()?;
        }
        Ok(())
    }

    pub(crate) fn publication_reported(
        &self,
        request: &DispatchRequest,
        credential: CredentialStamp,
    ) -> Result<(), StoreError> {
        let mut locked = self.lock()?;
        let saved = find(&mut locked.journal, request)?;
        if let Some(intent) = &mut saved.publication
            && intent.credential == credential
            && intent.accepted
        {
            intent.complete = true;
            locked.save()?;
        }
        Ok(())
    }
}

pub(crate) fn matching_summary<'a>(
    sent: &ThreadSummary,
    receipt: &'a ThreadSummariesReceipt,
) -> Option<&'a ThreadSummaryReceipt> {
    let mut matching = receipt
        .summaries
        .iter()
        .filter(|seen| seen.runner_id == sent.runner_id);
    let seen = matching.next()?;
    (!seen.id.is_nil() && seen.revision >= sent.revision && matching.next().is_none())
        .then_some(seen)
}

#[cfg(test)]
mod tests;
