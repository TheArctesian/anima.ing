//! Read-only view of the gates that hold new managed work.
use serde::Serialize;
use std::{path::Path, time::SystemTime};

use crate::{Error, RECEIPT, drain, pressure, recovery, require_protected_state, update_window};

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkAdmission {
    Maintenance,
    Reconciling,
    Draining,
    UpdateWindow,
    HostingPressure,
    PressureUnavailable,
    Clear,
}

/// Observe the local policy gates for new agent and build work.
///
/// The read takes no reservation or writer lock. Placement and dispatch still
/// evaluate the exact request against current policy and capacity.
///
/// # Errors
/// A gate could not be observed safely.
pub fn work_admission() -> Result<WorkAdmission, Error> {
    require_protected_state()?;
    if Path::new(RECEIPT).try_exists()? {
        return Ok(WorkAdmission::Maintenance);
    }
    if !recovery::ready()? {
        return Ok(WorkAdmission::Reconciling);
    }
    if drain::status()?.is_some() {
        return Ok(WorkAdmission::Draining);
    }
    if update_window::active_until(SystemTime::now())?.is_some() {
        return Ok(WorkAdmission::UpdateWindow);
    }
    Ok(match pressure::observe()? {
        pressure::Observation::Holding => WorkAdmission::HostingPressure,
        pressure::Observation::Stale => WorkAdmission::PressureUnavailable,
        pressure::Observation::Clear => WorkAdmission::Clear,
    })
}
