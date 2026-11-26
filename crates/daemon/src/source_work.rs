//! Durable dispatch identity and report delivery for the one enrolled work coordinator.
mod artifacts;
mod checkout;
mod claims;
pub(crate) use artifacts::Transfer;
pub(crate) use claims::{Selection, SourceOffer};
mod journal;
mod lifecycle;
mod reporting;
mod retirement;
mod terminal;
use crate::config::{home::Home, origin::Origin};
use daemon_core::{
    source_jobs::{Action, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, Request, SOCKET},
    wire::{Held, WorkJob, WorkReport},
};
use journal::{Entry, Journal};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Source-work storage is unavailable.")]
    Io(#[from] std::io::Error),
    #[error("Source-work storage is not private.")]
    Private(#[from] crate::config::private_file::PrivateFileError),
    #[error("Source-work state is invalid.")]
    Json(#[from] serde_json::Error),
    #[error("Source-work identity or state conflicts with the durable intent.")]
    Conflict,
    #[error("Source-work capacity is exhausted.")]
    Full,
    #[error("Another work coordinator owns this journal.")]
    Busy,
    #[error("The source owner did not confirm this operation; retry the same intent.")]
    Unconfirmed,
}

pub struct Coordinator {
    journal: Journal,
    device: Uuid,
    origin: String,
}

impl Coordinator {
    /// Acquires exclusive dispatch ownership; no hub or root-owner operation occurs.
    ///
    /// # Errors
    /// Refuses invalid enrollment, exposed/corrupt storage or another coordinator.
    pub fn open(home: &Home, device: Uuid, origin: &Origin) -> Result<Self, Error> {
        let origin = origin.as_text();
        if device.is_nil() || !origin.starts_with("https://") || origin.len() > 512 {
            return Err(Error::Conflict);
        }
        Ok(Self {
            journal: Journal::open(home)?,
            device,
            origin,
        })
    }

    /// Persists the intent before registration. Retries reuse the original body and UUID.
    /// Local repository permission is still checked by the root owner.
    ///
    /// # Errors
    /// Storage, conflicting retries, unsupported jobs or an unconfirmed broker reply.
    pub async fn register(&mut self, job: &WorkJob) -> Result<Uuid, Error> {
        let entry = self.journal.reserve(self.device, &self.origin, job)?;
        self.call(
            &entry,
            Action::Register {
                execution: entry.execution,
                item: serde_json::to_value(&entry.job)?,
            },
        )
        .await?;
        Ok(entry.execution)
    }

    /// All unacknowledged attempts, including terminal reports awaiting delivery.
    ///
    /// # Errors
    /// An earlier ambiguous storage failure requires reopening the journal.
    pub fn holds(&self) -> Result<Vec<Held>, Error> {
        self.journal.ready()?;
        Ok(self
            .journal
            .entries
            .iter()
            .filter(|entry| self.belongs(entry) && !entry.settled)
            .map(|entry| Held {
                id: entry.job.id,
                attempt: entry.job.attempt,
            })
            .collect())
    }

    /// Reports are retained until the caller confirms the exact hub-accepted body.
    ///
    /// # Errors
    /// An earlier ambiguous storage failure requires reopening the journal.
    pub fn pending(&self) -> Result<Vec<(Uuid, WorkReport)>, Error> {
        self.journal.ready()?;
        Ok(self
            .journal
            .entries
            .iter()
            .filter(|entry| self.belongs(entry))
            .filter_map(|entry| entry.report.clone().map(|report| (entry.execution, report)))
            .collect())
    }

    /// Stores a report before sending it. A final report cannot be replaced.
    ///
    /// # Errors
    /// Mismatched attempt, settled intent, conflicting final report or storage failure.
    pub fn report(&mut self, execution: Uuid, report: WorkReport) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        if entry.settled
            || report.id != entry.job.id
            || report.attempt != entry.job.attempt
            || report.log.is_some()
            || report.state.is_final() != report.result.is_some()
            || entry
                .report
                .as_ref()
                .is_some_and(|old| old.state.is_final() && old != &report)
        {
            return Err(Error::Conflict);
        }
        entry.report = Some(report);
        self.journal.replace(entry)
    }

    fn belongs(&self, entry: &Entry) -> bool {
        entry.device == self.device && entry.origin == self.origin
    }

    fn owned(&self, execution: Uuid) -> Result<&Entry, Error> {
        self.journal.ready()?;
        self.journal
            .entries
            .iter()
            .find(|entry| entry.execution == execution && self.belongs(entry))
            .ok_or(Error::Conflict)
    }

    async fn call(&self, entry: &Entry, action: Action) -> Result<Value, Error> {
        let request = Request {
            device: self.device,
            origin: self.origin.clone(),
            action,
        };
        let value = crate::runtime::owner_socket::call(
            SOCKET,
            &request,
            (MAX_REQUEST_BYTES, MAX_REPLY_BYTES),
        )
        .await
        .ok_or(Error::Unconfirmed)?;
        let mut job: WorkJob =
            serde_json::from_value(value["job"].clone()).map_err(|_| Error::Unconfirmed)?;
        job.lease = entry.job.lease;
        if value["execution"] != entry.execution.to_string()
            || value["device"] != self.device.to_string()
            || value["origin"] != self.origin
            || value["coordinated"] != true
            || job != entry.job
        {
            return Err(Error::Unconfirmed);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod socket_tests;
#[cfg(test)]
mod tests;
