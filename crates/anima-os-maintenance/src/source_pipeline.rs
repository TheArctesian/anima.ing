//! Source supervision remains a maintenance blocker between runtime reservations.
use crate::{Admission, Error, ROOT, drain, pressure, read_small_receipt, write_receipt};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use uuid::Uuid;

const PATH: &str = "/run/anima-os/source-pipeline.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    schema: u32,
    execution: Uuid,
    workspace: Uuid,
    build: Uuid,
}

impl Pipeline {
    /// Record acceptance under admission before dispatching the source supervisor.
    ///
    /// # Errors
    /// New work is held, another pipeline owns admission, or the record cannot be saved.
    pub fn accept(
        admission: &Admission,
        (execution, workspace, build): (Uuid, Uuid, Uuid),
    ) -> Result<(), Error> {
        drain::require_open(admission.opened_at)?;
        pressure::require_clear()?;
        let pipeline = Self {
            schema: 1,
            execution,
            workspace,
            build,
        };
        pipeline.validate()?;
        match active()? {
            Some(current) if current == pipeline => Ok(()),
            Some(_) => Err(Error::Refused(
                "Another source build is still being reconciled.",
            )),
            None => write_receipt(PATH, "source-pipeline.pending", &pipeline),
        }
    }

    /// Reopen the exact admission recorded by the supervisor.
    ///
    /// # Errors
    /// The current source pipeline is missing, invalid or belongs to another execution.
    pub fn resume(execution: Uuid) -> Result<Self, Error> {
        active()?
            .filter(|pipeline| pipeline.execution == execution)
            .ok_or(Error::Refused(
                "The source build no longer owns pipeline admission.",
            ))
    }

    pub(super) fn require_current(self) -> Result<(), Error> {
        if active()? != Some(self) {
            return Err(Error::Refused(
                "The source build no longer owns pipeline admission.",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn execution(self) -> Uuid {
        self.execution
    }

    #[must_use]
    pub const fn workspace(self) -> Uuid {
        self.workspace
    }

    #[must_use]
    pub const fn build(self) -> Uuid {
        self.build
    }

    fn validate(self) -> Result<(), Error> {
        if self.schema != 1
            || [self.execution, self.workspace, self.build]
                .iter()
                .any(Uuid::is_nil)
        {
            return Err(Error::Refused(
                "The source pipeline admission record is invalid.",
            ));
        }
        Ok(())
    }
}

/// Observe the single accepted pipeline for this boot.
///
/// # Errors
/// The admission record is inaccessible or invalid.
pub fn active() -> Result<Option<Pipeline>, Error> {
    let pipeline: Option<Pipeline> = read_small_receipt(PATH)?;
    if let Some(pipeline) = pipeline {
        pipeline.validate()?;
    }
    Ok(pipeline)
}

/// Retire the journal-proven terminal execution without clearing a newer pipeline.
///
/// # Errors
/// The record cannot be read or removed.
pub fn retire(_admission: &Admission, execution: Uuid) -> Result<(), Error> {
    if active()?.is_some_and(|current| current.execution == execution) {
        fs::remove_file(PATH)?;
        File::open(ROOT)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
