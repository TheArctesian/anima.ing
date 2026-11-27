//! Keep a created checkout container under its live source owner until dispatch.
use super::{Error, Grant, Phase, Record, Reservation, Result, record, runtime};
use crate::linux::source_jobs;
use anima_os_maintenance::{
    Admission, Error as AdmissionError, reservations, source_pipeline::Pipeline,
};
use std::{fs::File, time::Duration};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(super) struct SourceAuthority {
    pub pipeline: Pipeline,
    pub grant: Grant,
}

impl SourceAuthority {
    pub(super) fn authorize(self) -> Result<()> {
        source_jobs::require_checkout_lease(self.pipeline, self.grant)
    }

    async fn resume(self, state: &Record) -> Result<LockedWorkspace> {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            self.authorize()?;
            match LockedWorkspace::acquire(state.workspace, Some(self)) {
                Ok(locks) => {
                    let current = record::required(state.workspace)?;
                    current.require_launchable()?;
                    if current.phase != Phase::Starting
                        || current.principal.is_some()
                        || (current.generation, current.run, current.resources)
                            != (state.generation, state.run, state.resources)
                    {
                        return Err(Error::Refused("The held checkout workspace changed."));
                    }
                    return Ok(locks);
                }
                Err(Error::Admission(AdmissionError::Busy) | Error::WorkspaceBusy) => (),
                Err(error) => return Err(error),
            }
        }
    }
}

pub(super) struct LockedWorkspace {
    pub(super) admission: Admission,
    workspace: File,
    source: Option<SourceAuthority>,
}

impl LockedWorkspace {
    pub(super) fn acquire(id: Uuid, source: Option<SourceAuthority>) -> Result<Self> {
        Ok(Self {
            admission: Admission::for_pipeline(source.map(|authority| authority.pipeline))?,
            workspace: record::lock(id)?,
            source,
        })
    }

    pub(super) async fn dispatch(
        mut self,
        state: &mut Record,
        reservation: &Reservation,
    ) -> Result<File> {
        let prepared = runtime::prepare(state)
            .await
            .or_else(|error| interrupted(state, error))?;
        loop {
            match (
                reservations::reserve(&self.admission, reservation.clone()),
                self.source,
            ) {
                (Ok(_), _) => break,
                (
                    Err(AdmissionError::HostingPressure | AdmissionError::PressureUnavailable),
                    Some(source),
                ) => {
                    drop(self);
                    self = source.resume(state).await?;
                }
                (Err(error), _) => return interrupted(state, error.into()),
            }
        }
        if let Some(source) = self.source {
            source.authorize()?;
        }
        state.image = Some(
            prepared
                .start()
                .await
                .or_else(|error| interrupted(state, error))?,
        );
        Ok(self.workspace)
    }
}

fn interrupted<T>(state: &mut Record, error: Error) -> Result<T> {
    state.phase = Phase::Unknown;
    record::save(state)?;
    Err(error)
}
