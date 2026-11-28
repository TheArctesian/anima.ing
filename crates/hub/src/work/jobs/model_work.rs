use chrono::{DateTime, Utc};
use daemon_core::{
    model_work::{
        names::{Actor, Word},
        order::bypasses_switch_and_policy,
    },
    wire::{Claim, Lease, Limits, Switch, WorkArchitecture, WorkKind, WorkOs, WorkType},
};
use serde::Serialize;
use sqlx::PgConnection;

use super::{Claimed, Error, lifecycle, placement::Offer, store, trail};
use crate::{fleet::events::ClientAddr, ids::DeviceId};

/// The hardware allowances after applying the device policy.
#[derive(Clone, Copy)]
pub struct ModelBudget {
    pub memory_bytes: u64,
    pub disk_bytes: u64,
}

pub type ModelPolicy = for<'a> fn(
    &'a mut PgConnection,
    (DeviceId, bool),
    DateTime<Utc>,
) -> std::pin::Pin<
    Box<dyn Future<Output = Result<Option<ModelBudget>, Error>> + Send + 'a>,
>;

#[derive(Clone, Copy)]
pub(super) enum Admission {
    None,
    Reducing,
    All(ModelBudget),
}

pub(super) fn advertised(claim: &Claim) -> bool {
    claim.kinds.contains(&WorkKind::Model) && claim.types.iter().any(|kind| kind.is_model())
}

pub(super) fn exclusive(claim: &Claim) -> bool {
    advertised(claim) && claim.kinds.iter().all(|kind| *kind == WorkKind::Model)
}

pub(super) async fn admission(
    conn: &mut PgConnection,
    (claim, threads, policy): (&Claim, bool, Option<ModelPolicy>),
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Admission, Error> {
    if claim.unclean || !advertised(claim) {
        return Ok(Admission::None);
    }
    if claim.switch == Switch::Off {
        return Ok(if threads {
            Admission::Reducing
        } else {
            Admission::None
        });
    }
    if claim
        .types
        .iter()
        .copied()
        .all(|kind| !kind.is_model() || bypasses_switch_and_policy(kind))
    {
        return Ok(Admission::Reducing);
    }
    let apple_silicon =
        claim.os == WorkOs::Macos && claim.architecture == WorkArchitecture::Aarch64;
    let budget = match policy {
        Some(read) => read(conn, (device, apple_silicon), now).await?,
        None => return Ok(Admission::None),
    };
    Ok(budget.map_or(Admission::Reducing, Admission::All))
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ModelWork {
    id: uuid::Uuid,
    #[serde(rename = "type")]
    work_type: WorkType,
    kind: WorkKind,
    attempt: u32,
    lease: Lease,
    limits: Limits,
    project: Option<uuid::Uuid>,
    secrets: Vec<String>,
    parameters: serde_json::Value,
    actor: &'static str,
}

#[derive(Default)]
pub(super) struct Slots {
    mutation: bool,
    measure: bool,
}

impl Slots {
    pub(super) fn hold(&mut self, kind: WorkType) {
        match kind {
            WorkType::ModelMeasure => self.measure = true,
            WorkType::ModelDrain | WorkType::ModelReport => {}
            _ => self.mutation = true,
        }
    }

    fn available(&self, kind: WorkType) -> bool {
        match kind {
            WorkType::ModelMeasure => !self.measure,
            WorkType::ModelDrain | WorkType::ModelReport => true,
            _ => !self.mutation,
        }
    }
}

pub(super) async fn claim_next(
    conn: &mut PgConnection,
    (offer, exclusive): (&Offer, bool),
    (admission, slots): (Admission, &Slots),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Option<Claimed>, Error> {
    let types = |reducing| {
        offer
            .types
            .iter()
            .copied()
            .filter(|kind| kind.is_model() && bypasses_switch_and_policy(*kind) == reducing)
            .filter(|kind| slots.available(*kind))
            .map(WorkType::as_str)
            .collect::<Vec<_>>()
    };
    let (ordinary, budget) = match admission {
        Admission::None => return Ok(None),
        Admission::Reducing => (Vec::new(), None),
        Admission::All(budget) => (types(false), Some(budget)),
    };
    let Some((queued, actor)) =
        store::model_work::next(conn, (offer, exclusive), (&types(true), &ordinary), budget)
            .await?
    else {
        return Ok(None);
    };
    let actor = Actor::parse(&actor).ok_or(Error::Unreadable)?;
    let job = lifecycle::claimed(&queued, offer.device, now);
    store::save(conn, &job).await?;
    trail::model_started(conn, &job, (offer.device, actor), (client, now)).await?;
    Ok(Some(Claimed::Model(Box::new(ModelWork {
        id: job.id.into(),
        work_type: job.asked.work_type,
        kind: job.asked.kind,
        attempt: job.attempt,
        lease: Lease {
            until: job.lease_ends_at.ok_or(Error::Unreadable)?,
        },
        limits: job.asked.limits,
        project: None,
        secrets: job.asked.secrets,
        parameters: job.asked.parameters,
        actor: actor.as_str(),
    }))))
}
