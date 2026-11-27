use chrono::{DateTime, Utc};
use daemon_core::model_work::names::Word as _;
use daemon_core::wire::{Claim, Lease, Limits, Switch, WorkJob, WorkKind, WorkType};
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    Claimed, Error, Queue, ended, hosting, lifecycle,
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
    let work_off = placement::is_off(&machine, now);
    let slots = reconcile(conn, (device, claim, !work_off), (client, now)).await?;
    if hosting::can_take(claim)
        && let Some(host) =
            hosting::claim_next(conn, (&machine, slots.mutation, claim), (client, now)).await?
    {
        return Ok(host);
    }
    if work_off && claim.applications == Switch::Off && claim.switch == Switch::On {
        return Ok(Claimed::Off);
    }
    let room = Room::of(claim, &slots.holds);
    if work_off || room.is_empty() {
        return Ok(Claimed::Nothing);
    }
    let offer = Offer::of(claim, &machine, claimant.policy_permits);
    place(conn, (&offer, name, &room), (client, now)).await
}

/// What the machine still holds: its jobs and model items, and whether a host item takes the
/// one slot of the changes of the machine.
#[derive(Default)]
struct Slots {
    holds: Vec<Job>,
    mutation: bool,
}

async fn reconcile(
    conn: &mut PgConnection,
    (device, claim, jobs_on): (DeviceId, &Claim, bool),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Slots, Error> {
    let mut slots = Slots::default();
    for held in store::held_by(conn, device).await? {
        let host = held.asked.kind == WorkKind::Host;
        if !host && !jobs_on {
            continue;
        }
        if claim
            .holds
            .iter()
            .any(|each| JobId::from(each.id) == held.id && each.attempt == held.attempt)
        {
            if host {
                slots.mutation |= hosting::occupies_mutation(&held);
            } else {
                slots.holds.push(held);
            }
            continue;
        }
        let next = lifecycle::answer_lost(&held, now);
        store::save(conn, &next).await?;
        let reason = if host { "orphan" } else { "answer_lost" };
        trail::attempt_ended(conn, &next, (device, reason), (client, now)).await?;
    }
    Ok(slots)
}

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
    Ok(Claimed::Job(Box::new(work_of(&job)?)))
}

/// The job as the machine gets it; host work goes out by its own route.
fn work_of(job: &Job) -> Result<Work, Error> {
    let asked = &job.asked;
    let lease = Lease {
        until: job.lease_ends_at.unwrap_or(job.changed_at),
    };
    Ok(match &asked.target {
        Target::Host => return Err(Error::Unreadable),
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
    })
}

pub(in crate::work) mod checkout;
