use super::{Error, Result, store, supervised_lease};
use anima_os_maintenance::{
    Admission, Error as AdmissionError,
    reservations::{self, Reservation, RuntimeOwner},
    source_pipeline::Pipeline,
};
use std::{fs::File, time::Duration};
use uuid::Uuid;

pub(super) struct LockedAttempt {
    admission: Admission,
    _attempt: File,
}

impl LockedAttempt {
    pub(super) fn acquire(id: Uuid, supervisor: Option<Pipeline>) -> Result<Self> {
        Ok(Self {
            admission: Admission::for_pipeline(supervisor)?,
            _attempt: store::lock(id)?,
        })
    }

    pub(super) fn require_open_work(&self) -> Result<()> {
        Ok(self.admission.require_open_work()?)
    }

    pub(super) async fn reserve(
        mut self,
        attempt: &store::Attempt,
        supervisor: Option<Pipeline>,
    ) -> Result<Self> {
        let reservation = Reservation {
            schema: 1,
            runtime: RuntimeOwner::SystemdBuild,
            run: attempt.id,
            generation: 1,
            resources: attempt.resources,
        };
        loop {
            match (
                reservations::reserve(&self.admission, reservation.clone()),
                supervisor,
            ) {
                (Ok(_), _) => return Ok(self),
                (
                    Err(AdmissionError::HostingPressure | AdmissionError::PressureUnavailable),
                    Some(pipeline),
                ) => {
                    drop(self);
                    self = resume_preparation(attempt, pipeline).await?;
                }
                (Err(error), _) => return Err(error.into()),
            }
        }
    }
}

async fn resume_preparation(attempt: &store::Attempt, pipeline: Pipeline) -> Result<LockedAttempt> {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        supervised_lease(attempt, pipeline)?;
        match LockedAttempt::acquire(attempt.id, Some(pipeline)) {
            Ok(locks) => {
                if store::required(attempt.id)?.phase != "preparing" {
                    return Err(Error::Refused("The held build no longer owns preparation."));
                }
                return Ok(locks);
            }
            Err(Error::Admission(AdmissionError::Busy) | Error::BuildAttemptBusy) => (),
            Err(error) => return Err(error),
        }
    }
}
