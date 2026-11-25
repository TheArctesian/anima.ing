//! A runner UUID survives retries. Only a live exchange can move it past the dispatch marker.

use agent_threads::drive::Mode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::origin::Origin;

use super::{
    Store, StoreError,
    policy::{Refusal, lower},
    time::{Sample, sample},
    wire::{
        ClaimedLaunch, DispatchRequest, Evidence, EvidenceOutcome, Granted, LaunchParameters,
        NotCreated, Owner, Publication,
    },
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Reservation {
    owner: Owner,
    origin: String,
    request: DispatchRequest,
    project: Uuid,
    parameters: LaunchParameters,
    pub(super) state: LaunchState,
    #[serde(default)]
    pub(super) control: Option<super::command::SavedOffer>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) enum LaunchState {
    Reserved,
    Closed(NotCreated),
    Dispatching {
        grant: GrantReference,
        access: Mode,
    },
    Created {
        grant: GrantReference,
        access: Mode,
        process: ProcessIdentity,
        withdrawn: bool,
        ended: bool,
    },
}

/// One holder-observed lifetime, independent of a PID or a thread UUID.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity(Uuid);

impl ProcessIdentity {
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.0
    }
}

pub use dispatch::{Dispatch, Spawned};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct GrantReference {
    authorization: Uuid,
    valid_until: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Reserved,
    WaitingForGrant,
    Reconciling,
    Closed(NotCreated),
    Created,
}

/// Timing and correlation are local to this request, with no serialized form.
pub(super) struct DispatchExchange {
    request: DispatchRequest,
    start: Sample,
}

pub struct DispatchGrant {
    request: DispatchRequest,
    start: Sample,
    grant: Granted,
}

/// Timing sampled before the report that extends the lease by 90 seconds.
pub(super) struct PreparingReport(Sample);

/// An acknowledged preparing report with no stop order.
pub struct LiveLease {
    start: Sample,
    request: DispatchRequest,
}

impl Reservation {
    #[must_use]
    pub const fn request(&self) -> &DispatchRequest {
        &self.request
    }

    #[must_use]
    pub const fn project(&self) -> Uuid {
        self.project
    }

    #[must_use]
    pub const fn parameters(&self) -> &LaunchParameters {
        &self.parameters
    }

    #[must_use]
    pub const fn progress(&self) -> Progress {
        match self.state {
            LaunchState::Reserved => Progress::Reserved,
            LaunchState::Closed(cause) => Progress::Closed(cause),
            LaunchState::Dispatching { .. } => Progress::Reconciling,
            LaunchState::Created { .. } => Progress::Created,
        }
    }

    #[must_use]
    pub fn evidence(&self) -> Option<Evidence> {
        let outcome = match self.state {
            LaunchState::Closed(cause) => EvidenceOutcome::NotCreated { cause },
            LaunchState::Created { withdrawn, .. } => EvidenceOutcome::Created {
                thread: self.request.runner,
                cause: (),
                publication: withdrawn.then_some(Publication::Withdrawn),
            },
            LaunchState::Reserved | LaunchState::Dispatching { .. } => return None,
        };
        Some(Evidence {
            device: self.request.device,
            launch: self.request.launch,
            work: self.request.work,
            attempt: self.request.attempt,
            outcome,
        })
    }

    #[must_use]
    pub fn matches_process(&self, enrolled: &ProcessIdentity) -> bool {
        matches!(&self.state, LaunchState::Created { process, ended: false, .. } if process == enrolled)
    }

    #[must_use]
    pub const fn publication_open(&self) -> bool {
        matches!(
            self.state,
            LaunchState::Created {
                withdrawn: false,
                ..
            }
        )
    }

    pub(crate) fn enrolled_at(&self, device: Uuid, hub: &Origin) -> bool {
        self.owner.device == device && self.origin == hub.as_text()
    }

    pub(super) fn owns(&self, owner: &Owner, runner: Uuid) -> bool {
        self.owner == *owner && self.request.runner == runner
    }
}

impl PreparingReport {
    #[must_use]
    pub(super) fn begin() -> Option<Self> {
        sample().map(Self)
    }

    pub(super) fn accepted(self, request: DispatchRequest) -> LiveLease {
        LiveLease {
            start: self.0,
            request,
        }
    }
}

impl DispatchExchange {
    #[must_use]
    pub(super) fn begin(request: DispatchRequest) -> Option<Self> {
        Some(Self {
            request,
            start: sample()?,
        })
    }

    pub(super) fn accepted(self, grant: Granted) -> Option<DispatchGrant> {
        if !(1..=10_000).contains(&grant.remaining_ms) || grant.authorization.is_nil() {
            return None;
        }
        Some(DispatchGrant {
            request: self.request,
            start: self.start,
            grant,
        })
    }
}

impl DispatchGrant {
    fn live(&self, lease: &LiveLease, now: Sample) -> bool {
        self.request == lease.request
            && self.start.before(now, self.grant.remaining_ms)
            && lease.start.before(now, 90_000)
    }
}

impl Store {
    /// Reserve exactly one UUID per immutable launch and original claim tuple.
    ///
    /// # Errors
    /// Conflicting claim fields, or the reservation could not be stored durably.
    pub fn reserve(
        &self,
        (device, hub): (Uuid, &Origin),
        claim: &ClaimedLaunch,
    ) -> Result<Reservation, StoreError> {
        if !claim.valid() || device.is_nil() {
            return Err(StoreError::Invalid);
        }
        let launch = claim.parameters.launch;
        let owner = Owner {
            device,
            account: claim.parameters.account,
        };
        let mut locked = self.lock()?;
        if let Some(saved) = locked.journal.launches.get(&launch) {
            if !saved.enrolled_at(device, hub)
                || saved.owner != owner
                || saved.request.work != claim.id
                || saved.request.attempt != claim.attempt
                || saved.project != claim.project
                || saved.parameters != claim.parameters
            {
                return Err(StoreError::Conflict);
            }
            return Ok(saved.clone());
        }
        locked.journal.admit()?;
        let reservation = Reservation {
            request: DispatchRequest {
                device: owner.device,
                launch,
                work: claim.id,
                attempt: claim.attempt,
                runner: Uuid::now_v7(),
            },
            owner,
            origin: hub.as_text(),
            project: claim.project,
            parameters: claim.parameters.clone(),
            state: LaunchState::Reserved,
            control: None,
        };
        locked.journal.launches.insert(launch, reservation.clone());
        locked.save_admission()?;
        Ok(reservation)
    }

    /// Close only an unmarked reservation. Serialize with any dispatch of the same UUID.
    ///
    /// # Errors
    /// A conflicting tuple or a storage failure prevents this transition.
    pub fn close(
        &self,
        request: &DispatchRequest,
        cause: NotCreated,
    ) -> Result<Progress, StoreError> {
        self.transition(request, |state| {
            if matches!(state, LaunchState::Reserved) {
                *state = LaunchState::Closed(cause);
                true
            } else {
                false
            }
        })
    }

    /// Record publication withdrawal without changing the local process or permitting a later
    /// publication refresh to reopen remote control.
    ///
    /// # Errors
    /// A conflicting tuple or a storage failure prevents this transition.
    pub fn withdraw(&self, request: &DispatchRequest) -> Result<Progress, StoreError> {
        self.transition(request, |state| match state {
            LaunchState::Created { withdrawn, .. } if !*withdrawn => {
                *withdrawn = true;
                true
            }
            _ => false,
        })
    }

    /// The original launch bindings for this enrollment, including pending evidence.
    ///
    /// # Errors
    /// The durable journal could not be read.
    pub fn enrolled_reservations(
        &self,
        device: Uuid,
        hub: &Origin,
    ) -> Result<Vec<Reservation>, StoreError> {
        let locked = self.lock()?;
        Ok(locked
            .journal
            .launches
            .values()
            .filter(|row| row.enrolled_at(device, hub))
            .cloned()
            .collect())
    }

    /// The original reservation and any evidence that survived restart.
    ///
    /// # Errors
    /// The original tuple is missing or storage failed.
    pub fn reservation(&self, request: &DispatchRequest) -> Result<Reservation, StoreError> {
        let mut locked = self.lock()?;
        Ok(find(&mut locked.journal, request)?.clone())
    }

    fn transition(
        &self,
        request: &DispatchRequest,
        change: impl FnOnce(&mut LaunchState) -> bool,
    ) -> Result<Progress, StoreError> {
        let mut locked = self.lock()?;
        let saved = find(&mut locked.journal, request)?;
        let changed = change(&mut saved.state);
        let progress = saved.progress();
        if changed {
            locked.save()?;
        }
        Ok(progress)
    }
}

pub(super) fn find<'a>(
    journal: &'a mut super::journal::Journal,
    request: &DispatchRequest,
) -> Result<&'a mut Reservation, StoreError> {
    journal
        .launches
        .get_mut(&request.launch)
        .filter(|saved| saved.request == *request)
        .ok_or(StoreError::Conflict)
}

mod dispatch;
#[cfg(test)]
pub(super) use dispatch::tests::started_fixture;
