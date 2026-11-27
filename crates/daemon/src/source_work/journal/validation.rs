//! Validate persisted attempt ownership and the closed source-build request.
use super::{Disposition, Error, LIMIT, Snapshot};
use daemon_core::{
    source_jobs::private_checkout,
    wire::{WorkJob, WorkKind, WorkType},
    work::{
        checks::{self, Checked, Machine},
        permitted::Repositories,
        types::Version,
    },
};

pub(super) fn validate(snapshot: &Snapshot) -> Result<(), Error> {
    if snapshot.schema != 1 || snapshot.entries.len() > LIMIT {
        return Err(Error::Conflict);
    }
    for (index, entry) in snapshot.entries.iter().enumerate() {
        validate_job(&entry.job)?;
        if entry.renewal.is_some_and(|grant| {
            grant.job != entry.job.id
                || grant.attempt != entry.job.attempt
                || grant.until < entry.job.lease.until
        }) {
            return Err(Error::Conflict);
        }
        if entry.execution.is_nil()
            || entry.device.is_nil()
            || entry.origin.len() > 512
            || crate::config::origin::Origin::parse(&entry.origin).is_err()
            || !entry.origin.starts_with("https://")
            || entry.artifact_receipt.as_ref().is_some_and(|receipt| {
                !receipt.valid()
                    || receipt.artifact.job != entry.job.id
                    || receipt.artifact.attempt != entry.job.attempt
                    || receipt.device != entry.device
            })
            || (entry.disposition == Disposition::Lost && !entry.stop_requested)
            || (entry.checkout_requested_at.is_some() && !private_checkout(&entry.job))
            || (entry.settled && entry.report.is_some())
            || entry.report.as_ref().is_some_and(|report| {
                report.id != entry.job.id
                    || report.attempt != entry.job.attempt
                    || report.log.is_some()
                    || report.state.is_final() != report.result.is_some()
            })
            || snapshot.entries[..index].iter().any(|old| {
                old.execution == entry.execution
                    || (old.device == entry.device
                        && old.origin == entry.origin
                        && old.job.id == entry.job.id
                        && old.job.attempt == entry.job.attempt)
            })
        {
            return Err(Error::Conflict);
        }
    }
    Ok(())
}

pub(super) fn validate_job(job: &WorkJob) -> Result<(), Error> {
    if job.id.is_nil()
        || job.attempt == 0
        || job.work_type != WorkType::Build
        || job.kind != WorkKind::Container
        || (!job.secrets.is_empty() && !private_checkout(job))
        || job.parameters["recipe"] != "source_image_v1"
    {
        return Err(Error::Conflict);
    }
    // Only wire validation here. This list grants no permission; root owns admission.
    let repositories =
        Repositories::parse(&format!("{}/{}", job.repository.owner, job.repository.name))
            .map_err(|_| Error::Conflict)?;
    let machine = Machine {
        switch_on: true,
        policy_permits: true,
        kinds: &[WorkKind::Container],
        isolation: &[WorkKind::Container],
        unclean: false,
        repositories: &repositories,
        lowest_runner: Version(0, 0, 0),
    };
    match checks::check(&serde_json::to_value(job)?, &machine) {
        Checked::Start(_) => Ok(()),
        Checked::Refuse { .. } | Checked::Drop => Err(Error::Conflict),
    }
}
