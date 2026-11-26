//! K10 admission and dispatch for an exact container action on one enrolled machine.

use chrono::{DateTime, Utc};
use daemon_core::wire::{
    Claim, ContainerAction, HostItem, Lease, Limits, Switch, WorkKind, WorkOs, WorkType,
};
use sqlx::PgConnection;

use super::{Claimed, Error, Job, Machine, NewJob, State, lifecycle, store};
use crate::{
    fleet::{
        devices,
        events::{self, ClientAddr, Operator},
    },
    ids::DeviceId,
};

/// Queue an exact Docker action on the named device after the caller authorizes the write.
///
/// # Errors
/// Invalid actor, target or action, duplicate or throttled submission, or storage failure.
pub async fn enqueue_container_action(
    queue: super::Queue<'_>,
    (device, action): (DeviceId, &ContainerAction),
    asker: super::Asker,
    now: DateTime<Utc>,
) -> Result<Job, Error> {
    let parameters = serde_json::to_value(action).map_err(|_| Error::Unreadable)?;
    super::enqueue(
        queue,
        NewJob {
            work_type: WorkType::ContainerAction,
            kind: WorkKind::Host,
            os: Some(WorkOs::Linux),
            architecture: None,
            project: None,
            repository: None,
            limits: Limits {
                seconds: 120,
                idle_seconds: 1,
                memory_bytes: 1,
                disk_bytes: 1,
                orphan_seconds: 1,
            },
            secrets: Vec::new(),
            parameters,
            machine: Some(device),
        },
        asker,
        now,
    )
    .await
}

pub(super) fn validate(job: &NewJob) -> Result<(), Error> {
    let action = serde_json::from_value::<ContainerAction>(job.parameters.clone())
        .ok()
        .filter(ContainerAction::full_id);
    if job.work_type != WorkType::ContainerAction
        || job.machine.is_none()
        || job.project.is_some()
        || job.os.is_some_and(|os| os != WorkOs::Linux)
        || job.architecture.is_some()
        || job.repository.is_some()
        || !job.secrets.is_empty()
        || job.limits.seconds > 120
        || action.is_none()
    {
        return Err(Error::Invalid(
            "the container action must name one machine and a full container ID",
        ));
    }
    Ok(())
}

pub(super) async fn admit<'a>(
    conn: &mut PgConnection,
    job: &Job,
    operator: &'a Operator,
    now: DateTime<Utc>,
) -> Result<Option<&'a str>, Error> {
    use events::TokenKind;
    let key = match operator.actor.token() {
        Some(TokenKind::Login | TokenKind::Browser) => None,
        Some(TokenKind::ApiKey) => Some(
            operator
                .actor
                .api_key_name()
                .ok_or(Error::Invalid("the API key has no name"))?,
        ),
        _ => {
            return Err(Error::Invalid(
                "container actions require a login or API key",
            ));
        }
    };
    let device = job
        .asked
        .machine
        .ok_or(Error::Invalid("the applications machine is missing"))?;
    devices::lock(conn, device)
        .await
        .map_err(|error| match error {
            devices::Error::NotFound(_) => {
                Error::Invalid("the applications machine does not exist")
            }
            devices::Error::Database(error) => Error::Database(error),
            _ => Error::Unreadable,
        })?;
    if store::hosting::duplicate(conn, device, (job, now)).await? {
        return Err(Error::Invalid("an equal host work item is already waiting"));
    }
    if let Some(key) = key
        && !store::hosting::key_has_room(conn, device, key, now).await?
    {
        return Err(Error::Invalid(
            "the API key reached its container work limit for this machine",
        ));
    }
    Ok(key)
}

pub(super) fn can_take(claim: &Claim) -> bool {
    claim.os == WorkOs::Linux
        && claim.applications == Switch::On
        && claim
            .types
            .iter()
            .any(|kind| matches!(kind, WorkType::ContainerAction | WorkType::ApplicationApply))
        && claim.kinds.contains(&WorkKind::Host)
}

pub(super) fn occupies_mutation(job: &Job) -> bool {
    matches!(
        job.state,
        State::Claimed | State::Preparing | State::Running
    ) && job.asked.parameters["action"] != "stop"
}

pub(super) async fn claim_next(
    conn: &mut PgConnection,
    (machine, occupied, claim): (&Machine, bool, &Claim),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Option<Claimed>, Error> {
    let allow_mutation = !occupied && !machine.off_by_operator;
    let Some(chosen) =
        store::hosting::next(conn, machine.device, (claim, allow_mutation, now)).await?
    else {
        return Ok(None);
    };
    let job = lifecycle::claimed(&chosen, machine.device, now);
    store::save(conn, &job).await?;
    super::trail::host_started(conn, &job, (machine.device, client, now)).await?;
    Ok(Some(Claimed::Host(Box::new(wire_item(&job)))))
}

fn wire_item(job: &Job) -> HostItem {
    HostItem {
        id: job.id.into(),
        work_type: job.asked.work_type,
        kind: job.asked.kind,
        attempt: job.attempt,
        lease: Lease {
            until: job.lease_ends_at.unwrap_or(job.changed_at),
        },
        limits: Limits {
            seconds: job.asked.limits.seconds,
            idle_seconds: 0,
            memory_bytes: 0,
            disk_bytes: 0,
            orphan_seconds: 0,
        },
        project: None,
        secrets: job.asked.secrets.clone(),
        parameters: job.asked.parameters.clone(),
    }
}

pub(super) async fn expire_waiting(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<usize, Error> {
    let waiting = store::hosting::unclaimed_expired(conn, now).await?;
    for job in &waiting {
        let device = job.asked.machine.ok_or(Error::Unreadable)?;
        let end = lifecycle::orphan(job, now);
        store::save(conn, &end).await?;
        let at = (super::trail::HUB_ADDRESS, now);
        super::trail::attempt_ended(conn, &end, (device, "orphan"), at).await?;
    }
    Ok(waiting.len())
}
