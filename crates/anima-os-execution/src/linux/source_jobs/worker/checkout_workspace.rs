use super::{AdmissionError, Error, Grant, Pipeline, Resources, Result, workspace};
use crate::linux::source_jobs::require_checkout_lease;
use anima_os_maintenance::reservations::{Reservation, RuntimeOwner};
use std::time::Duration;

pub(super) async fn start(
    pipeline: Pipeline,
    grant: Grant,
    resources: Resources,
    storage: u64,
) -> Result<()> {
    let reservation = Reservation {
        schema: 1,
        runtime: RuntimeOwner::Docker,
        run: pipeline.execution(),
        generation: 1,
        resources,
    };
    loop {
        require_checkout_lease(pipeline, grant)?;
        match prepare_and_start(pipeline, grant, storage, reservation.clone()).await {
            Err(
                Error::Admission(
                    AdmissionError::HostingPressure
                    | AdmissionError::PressureUnavailable
                    | AdmissionError::Busy,
                )
                | Error::WorkspaceBusy,
            ) => tokio::time::sleep(Duration::from_secs(1)).await,
            result => return result,
        }
    }
}

async fn prepare_and_start(
    pipeline: Pipeline,
    grant: Grant,
    storage: u64,
    reservation: Reservation,
) -> Result<()> {
    workspace::prepare_source(pipeline, storage).await?;
    workspace::start_source(pipeline, reservation, grant)
        .await
        .map(drop)
}
