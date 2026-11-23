//! The claim of a machine (K10 Section 4): what the machine said is recorded, the attempts
//! whose answer was lost end, and the placement chooses the job.

use chrono::{DateTime, Utc};
use daemon_core::model_work::names::Word as _;
use daemon_core::wire::{Claim, Lease, Limits, Switch, WorkJob, WorkKind, WorkType};
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    Claimed, Error, Queue, ended, lifecycle,
    model::{Job, Machine, Target},
    placement::{self, Around, Offer, Room},
    store, trail,
};
use crate::{
    fleet::events::{Bus, ClientAddr},
    ids::{DeviceId, JobId},
};

/// The machine that asks, as the claim route knows it beside its claim.
#[derive(Clone, Copy, Debug)]
pub struct Claimant<'a> {
    pub device: DeviceId,
    /// The name of the device, for the decisions and the audit entries.
    pub name: &'a str,
    /// The device policy permits model work now (CAP-010 to CAP-012). The wiring reads it, so
    /// this domain imports no policy.
    pub policy_permits: bool,
}

/// What a machine gets: a job of K10, or a model item of K11, which has no `repository`, a
/// null `project` and the member `actor` (K11 Section 2).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Work {
    Job(WorkJob),
    Model(ModelWork),
}

/// A model item as the machine gets it (K11 Section 2).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelWork {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub work_type: WorkType,
    pub kind: WorkKind,
    pub attempt: u32,
    pub lease: Lease,
    pub limits: Limits,
    /// Always null for a model item.
    pub project: Option<Uuid>,
    pub secrets: Vec<String>,
    pub parameters: serde_json::Value,
    pub actor: &'static str,
}

/// Answers the claim of `claimant` at `now` (K10 Section 4): records what the machine said,
/// ends the attempts whose answer was lost, and gives the machine the item that the placement
/// chooses. `claim.device` is already known to be the device of `claimant`.
///
/// # Errors
/// `Database` or `Events` when a change cannot be written; nothing is changed then.
pub async fn claim(
    queue: Queue<'_>,
    claimant: Claimant<'_>,
    claim: &Claim,
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Claimed, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = claim_in(&mut tx, queue.bus, (claimant, claim), (client, now)).await;
    ended(tx, outcome).await
}

async fn claim_in(
    conn: &mut PgConnection,
    bus: &Bus,
    (claimant, claim): (Claimant<'_>, &Claim),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Claimed, Error> {
    let (device, name) = (claimant.device, claimant.name);
    let known = store::machine_locked(conn, device).await?;
    let first_on = claim.switch == Switch::On && known.is_none_or(|old| old.first_on_at.is_none());
    let machine = Machine {
        device,
        switch: claim.switch,
        unclean: claim.unclean,
        off_by_operator: known.is_some_and(|old| old.off_by_operator),
        off_until: known.and_then(|old| old.off_until),
        first_on_at: known
            .and_then(|old| old.first_on_at)
            .or(first_on.then_some(now)),
        excluded: known.is_some_and(|old| old.excluded),
    };
    store::save_machine(conn, &machine, now).await?;
    store::save_said(conn, (device, name), claim).await?;
    if first_on {
        trail::first_claim(conn, bus, (device, name), now).await?;
    }
    if placement::is_off(&machine, now) {
        return Ok(Claimed::Off);
    }
    let mut holds = Vec::new();
    for held in store::held_by(conn, device).await? {
        let named = claim
            .holds
            .iter()
            .any(|each| JobId::from(each.id) == held.id && each.attempt == held.attempt);
        if named {
            holds.push(held);
        } else {
            // The answer to the claim of this attempt was lost (SF-011).
            let back = lifecycle::answer_lost(&held, now);
            store::save(conn, &back).await?;
            trail::attempt_ended(conn, &back, (device, "answer_lost"), (client, now)).await?;
        }
    }
    let room = Room::of(claim, &holds);
    if room.is_empty() {
        return Ok(Claimed::Nothing);
    }
    let offer = Offer::of(claim, &machine, claimant.policy_permits);
    place(conn, (&offer, name, &room), (client, now)).await
}

/// Decides each queued item that `room` admits for the machine of `offer`, records each
/// decision, and gives the machine the item that the placement chooses, in the order of K11
/// Section 3, with the audit entry of the decision (ROU-005).
async fn place(
    conn: &mut PgConnection,
    (offer, name, room): (&Offer, &str, &Room),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Claimed, Error> {
    let mut queued = store::oldest_queued(conn).await?;
    queued.retain(|job| room.admits(job));
    queued.sort_by_key(placement::rank);
    let pins = store::pins(conn).await?;
    let asking = (now - placement::FRESH, now);
    let others = store::others_asking(conn, offer.device, asking).await?;
    let around = Around {
        pins: &pins,
        others: &others,
        now,
    };
    let verdicts: Vec<_> = queued
        .iter()
        .map(|job| placement::verdict(job, offer, &around))
        .collect();
    let decided: Vec<_> = queued
        .iter()
        .map(|job| job.id)
        .zip(verdicts.iter().copied())
        .collect();
    store::save_decisions(conn, (offer, name), &decided, now).await?;
    let Some(chosen) = placement::choose(&queued, &verdicts, offer.device) else {
        return Ok(Claimed::Nothing);
    };
    let job = lifecycle::claimed(chosen, offer.device, now);
    store::save(conn, &job).await?;
    let decisions = store::decisions_of(conn, job.id).await?;
    trail::attempt_started(conn, &job, (offer.device, &decisions), (client, now)).await?;
    Ok(Claimed::Job(Box::new(work_of(&job))))
}

/// The job as the machine gets it.
fn work_of(job: &Job) -> Work {
    let asked = &job.asked;
    let lease = Lease {
        until: job.lease_ends_at.unwrap_or(job.changed_at),
    };
    match &asked.target {
        Target::Repository(repository) => Work::Job(WorkJob {
            id: job.id.into(),
            work_type: asked.work_type,
            kind: asked.kind,
            attempt: job.attempt,
            lease,
            limits: asked.limits,
            project: asked.project.map(Uuid::from),
            repository: repository.clone(),
            secrets: asked.secrets.clone(),
            parameters: asked.parameters.clone(),
        }),
        Target::Model { actor } => Work::Model(ModelWork {
            id: job.id.into(),
            work_type: asked.work_type,
            kind: asked.kind,
            attempt: job.attempt,
            lease,
            limits: asked.limits,
            project: None,
            secrets: asked.secrets.clone(),
            parameters: asked.parameters.clone(),
            actor: actor.as_str(),
        }),
    }
}
