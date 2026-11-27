//! Which job a machine gets, and why (PRO-051, USE-003, USE-007; ROU-004 to ROU-007, R-259).

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::{
    Claim, REPOSITORY_TOKEN, SOURCE_IMAGE_PRIVATE_CHECKOUT, Switch, WorkArchitecture, WorkKind,
    WorkOs, WorkType,
};

use super::model::{Job, Machine};
use crate::ids::{DeviceId, ProjectId};

/// Faults in [`FAULT_WINDOW`] before a machine gets no work for [`OFF_FOR`] (K10 Section 11).
pub(super) const MOST_FAULTS: usize = 5;
pub(super) const FAULT_WINDOW: TimeDelta = TimeDelta::hours(1);
pub(super) const OFF_FOR: TimeDelta = TimeDelta::hours(1);

/// How long a new job in the queue waits for a machine with more free memory (R-259).
pub(super) const HOLD: TimeDelta = TimeDelta::seconds(20);
/// The most age of the last claim of a machine that a job can wait for.
pub(super) const FRESH: TimeDelta = TimeDelta::seconds(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub build_recipes: Option<daemon_core::wire::BuildRecipes>,
    pub device: DeviceId,
    pub types: Vec<WorkType>,
    pub kinds: Vec<WorkKind>,
    /// `None` for a word that this hub does not know: it fits no job that asks for a system.
    pub os: Option<WorkOs>,
    pub architecture: Option<WorkArchitecture>,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub excluded: bool,
}

impl Offer {
    pub(super) fn of(claim: &Claim, machine: &Machine) -> Self {
        Self {
            build_recipes: claim.build_recipes.clone(),
            device: machine.device,
            types: claim.types.clone(),
            kinds: claim.kinds.clone(),
            os: Some(claim.os),
            architecture: Some(claim.architecture),
            memory_bytes: claim.free.memory_bytes,
            disk_bytes: claim.free.disk_bytes,
            excluded: machine.excluded,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Fits,
    Type,
    Recipe,
    Kind,
    Os,
    Architecture,
    Memory,
    Disk,
    RefusedBefore,
    /// The job, or the pin of its project, names a different machine (ROU-006).
    NamesOther,
    /// The operator excluded the machine, and the job names no machine (ROU-006).
    Excluded,
    /// The machine fits, but a machine with more free memory asked a short time ago (R-259).
    WaitsForBetter,
}

impl Verdict {
    pub(super) const ALL: [Self; 12] = [
        Self::Fits,
        Self::Type,
        Self::Recipe,
        Self::Kind,
        Self::Os,
        Self::Architecture,
        Self::Memory,
        Self::Disk,
        Self::RefusedBefore,
        Self::NamesOther,
        Self::Excluded,
        Self::WaitsForBetter,
    ];

    /// The verdict as the database and the audit entries spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fits => "fits",
            Self::Type => "type",
            Self::Recipe => "recipe",
            Self::Kind => "kind",
            Self::Os => "os",
            Self::Architecture => "architecture",
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::RefusedBefore => "refused_before",
            Self::NamesOther => "names_other",
            Self::Excluded => "excluded",
            Self::WaitsForBetter => "waits_for_better",
        }
    }

    pub(super) fn from_stored(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|each| each.as_str() == text)
    }
}

pub(super) struct Around<'a> {
    pub pins: &'a HashMap<ProjectId, DeviceId>,
    pub others: &'a [Offer],
    pub now: DateTime<Utc>,
}

pub(super) fn is_off(machine: &Machine, now: DateTime<Utc>) -> bool {
    machine.off_by_operator || machine.off_until.is_some_and(|until| now < until)
}

pub(super) fn off_until(faults: usize, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    (faults >= MOST_FAULTS).then(|| now + OFF_FOR)
}

/// Whether a claim can get a job at all (K10 Section 4).
pub(super) fn can_take(claim: &Claim) -> bool {
    claim.switch == Switch::On && !claim.unclean && claim.free.slots >= 1
}

/// The job's own machine comes before its project's pin (ROU-006).
pub(super) fn named(job: &Job, pins: &HashMap<ProjectId, DeviceId>) -> Option<DeviceId> {
    let pinned = || job.asked.project.and_then(|project| pins.get(&project));
    if needs_private_checkout(&job.asked) {
        return pinned().copied();
    }
    job.asked.machine.or_else(|| pinned().copied())
}

pub(super) fn needs_private_checkout(asked: &super::NewJob) -> bool {
    asked.work_type == WorkType::Build
        && asked.parameters["recipe"] == "source_image_v1"
        && asked
            .secrets
            .iter()
            .any(|purpose| purpose == REPOSITORY_TOKEN)
}

/// Whether the machine of `offer` can run `job`, with no look at a different machine.
pub(super) fn alone(job: &Job, named: Option<DeviceId>, offer: &Offer) -> Verdict {
    let asked = &job.asked;
    let checks = [
        (
            named.map_or_else(
                || !needs_private_checkout(asked),
                |device| device == offer.device,
            ),
            Verdict::NamesOther,
        ),
        (named.is_some() || !offer.excluded, Verdict::Excluded),
        (offer.types.contains(&asked.work_type), Verdict::Type),
        (recipe_fits(job, offer), Verdict::Recipe),
        (offer.kinds.contains(&asked.kind), Verdict::Kind),
        (asked.os.is_none_or(|os| Some(os) == offer.os), Verdict::Os),
        (
            asked
                .architecture
                .is_none_or(|architecture| Some(architecture) == offer.architecture),
            Verdict::Architecture,
        ),
        (
            asked.limits.memory_bytes <= offer.memory_bytes,
            Verdict::Memory,
        ),
        (asked.limits.disk_bytes <= offer.disk_bytes, Verdict::Disk),
        (
            !job.refused_by.contains(&offer.device),
            Verdict::RefusedBefore,
        ),
    ];
    checks
        .into_iter()
        .find_map(|(holds, verdict)| (!holds).then_some(verdict))
        .unwrap_or(Verdict::Fits)
}

/// Free memory is a tie-break only, for [`HOLD`] after the job was queued (R-259, ROU-002).
pub(super) fn verdict(job: &Job, offer: &Offer, around: &Around<'_>) -> Verdict {
    let named = named(job, around.pins);
    let own = alone(job, named, offer);
    let new = around.now - job.changed_at < HOLD;
    let better = |other: &Offer| {
        other.device != offer.device
            && other.memory_bytes > offer.memory_bytes
            && alone(job, named, other) == Verdict::Fits
    };
    if own == Verdict::Fits && new && around.others.iter().any(better) {
        return Verdict::WaitsForBetter;
    }
    own
}

/// A job that a machine lost goes to a different machine when one asks first (USE-007).
pub(super) fn choose<'a>(
    queued: &'a [Job],
    verdicts: &[Verdict],
    device: DeviceId,
) -> Option<&'a Job> {
    let mut fitting = queued
        .iter()
        .zip(verdicts)
        .filter_map(|(job, verdict)| (*verdict == Verdict::Fits).then_some(job));
    let first = fitting.next()?;
    if first.last_device != Some(device) {
        return Some(first);
    }
    fitting
        .find(|job| job.last_device != Some(device))
        .or(Some(first))
}

/// `None` also when only free memory or disk is too small now, which can change (ROU-006, JOB-014).
pub(super) fn never_fits(
    job: &Job,
    known: Option<(&Machine, &Offer)>,
    now: DateTime<Utc>,
) -> Option<Unfit> {
    let Some((machine, offer)) = known else {
        return Some(Unfit::NeverAsked);
    };
    if machine.switch == Switch::Off || is_off(machine, now) {
        return Some(Unfit::TakesNoWork);
    }
    let fixed = [
        Verdict::Type,
        Verdict::Kind,
        Verdict::Os,
        Verdict::Architecture,
    ];
    let found = alone(job, Some(offer.device), offer);
    fixed.contains(&found).then_some(Unfit::Requirement(found))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    NeverAsked,
    TakesNoWork,
    Requirement(Verdict),
}

pub(super) fn recipe_fits(job: &Job, offer: &Offer) -> bool {
    if job.asked.work_type != WorkType::Build {
        return true;
    }
    job.asked.parameters["recipe"]
        .as_str()
        .is_some_and(|recipe| {
            offer
                .build_recipes
                .as_ref()
                .map_or(recipe == "desktop_release", |recipes| {
                    recipes.supports(recipe)
                        && (!needs_private_checkout(&job.asked)
                            || recipes.supports(SOURCE_IMAGE_PRIVATE_CHECKOUT))
                })
        })
}
