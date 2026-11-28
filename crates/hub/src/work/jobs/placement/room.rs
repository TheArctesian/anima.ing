//! What a claim can still get from the items that its machine holds (K10 Section 4, K11 Section 3).

use daemon_core::wire::{Claim, Switch, WorkKind, WorkType};

use super::{super::model::Job, can_take};

/// The model types that change the machine: it holds at most one at a time (K11 Section 3).
const CHANGING: [WorkType; 9] = [
    WorkType::RuntimeInstall,
    WorkType::ModelPull,
    WorkType::CopyDelete,
    WorkType::CopyAdopt,
    WorkType::ServerStart,
    WorkType::ServerStop,
    WorkType::ModelLoad,
    WorkType::ModelUnload,
    WorkType::ModelKeep,
];

/// What a claim can still get, from the claim and the items that the machine holds. A job
/// takes the slot of K10; a model item does not (K11 Section 3): the hub counts its own leases,
/// one item that changes the machine and one `model_measure` at a time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::work::jobs) struct Room {
    job: bool,
    model: bool,
    /// The types of the model items that the machine holds.
    held: Vec<WorkType>,
}

impl Room {
    pub(in crate::work::jobs) fn of(claim: &Claim, holds: &[Job]) -> Self {
        let (model, other): (Vec<&Job>, Vec<&Job>) = holds
            .iter()
            .partition(|job| job.asked.kind == WorkKind::Model);
        Self {
            job: can_take(claim) && other.is_empty(),
            model: claim.switch == Switch::On,
            held: model.iter().map(|job| job.asked.work_type).collect(),
        }
    }

    /// Whether the claim can get nothing.
    pub(in crate::work::jobs) const fn is_empty(&self) -> bool {
        !self.job && !self.model
    }

    /// Whether the claim can get `job`.
    pub(in crate::work::jobs) fn admits(&self, job: &Job) -> bool {
        if job.asked.kind != WorkKind::Model {
            return self.job;
        }
        let work_type = job.asked.work_type;
        let one_at_a_time =
            |test: fn(&WorkType) -> bool| test(&work_type) && self.held.iter().any(test);
        self.model
            && !one_at_a_time(|each| CHANGING.contains(each))
            && !one_at_a_time(|each| *each == WorkType::ModelMeasure)
    }
}
