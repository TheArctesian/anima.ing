//! The job owner's short authorization transaction for private source output.
use super::{Error, State, store};
use crate::{
    clock::{Clock, lease},
    ids::{DeviceId, JobId},
};
use chrono::{DateTime, Utc};
use daemon_core::wire::{Switch, WorkKind, WorkType};
use sqlx::{PgPool, Postgres, Transaction};

pub enum ArtifactAccess {
    Granted(ArtifactGuard),
    NotHolder,
    Off,
}

/// Keeps ownership stable only during the final storage commit, never while streaming.
pub struct ArtifactGuard {
    transaction: Transaction<'static, Postgres>,
    lease_end: DateTime<Utc>,
}

impl ArtifactGuard {
    #[must_use]
    pub const fn valid_until(&self) -> DateTime<Utc> {
        self.lease_end
    }

    /// Releases the row locks, refusing acknowledgement if the lease expired meanwhile.
    ///
    /// # Errors
    /// The database cannot commit or roll back the authorization transaction.
    pub async fn finish(self, now: DateTime<Utc>) -> Result<bool, Error> {
        if now >= self.lease_end {
            self.transaction.rollback().await?;
            return Ok(false);
        }
        self.transaction.commit().await?;
        Ok(true)
    }
}

/// Checks current source-job ownership under the same lock order as reporting.
/// Call `finish` before streaming, then acquire a fresh guard before retaining output.
///
/// # Errors
/// Database failure or an unreadable stored job.
pub async fn artifact_access(
    db: &PgPool,
    device: DeviceId,
    (id, attempt): (JobId, u32),
    clock: &dyn Clock,
) -> Result<ArtifactAccess, Error> {
    let mut transaction = db.begin().await?;
    let Some(machine) = store::machine_locked(&mut transaction, device).await? else {
        return Ok(ArtifactAccess::NotHolder);
    };
    let Some(job) = store::job_locked(&mut transaction, id).await? else {
        return Ok(ArtifactAccess::NotHolder);
    };
    let now = clock.now();
    if machine.switch != Switch::On || machine.unclean || machine.held_off(now) {
        return Ok(ArtifactAccess::Off);
    }
    if job.holder != Some(device)
        || job.attempt != attempt
        || job.state != State::Finishing
        || job.stop.is_some()
        || super::lifecycle::overdue(&job, now)
        || lease::ended(job.lease_ends_at, now)
        || job.asked.work_type != WorkType::Build
        || job.asked.kind != WorkKind::Container
        || job.asked.parameters["recipe"] != "source_image_v1"
        || job.asked.clone().bind_source_target().is_err()
    {
        return Ok(ArtifactAccess::NotHolder);
    }
    Ok(ArtifactAccess::Granted(ArtifactGuard {
        transaction,
        lease_end: job.lease_ends_at.ok_or(Error::Unreadable)?,
    }))
}
