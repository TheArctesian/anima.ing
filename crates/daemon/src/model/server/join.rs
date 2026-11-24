//! The wait of a start for the join of the launcher to the cgroup of its server (Linux).

use std::time::Duration;

use daemon_core::model_work::cause::{Cause, Containment};

use super::Running;
use crate::model::tree;

pub(super) const JOIN: Duration = Duration::from_secs(5);

/// The launcher joins before it `exec`s the runtime, so until the join the tree reads empty.
pub(super) async fn joined(mut running: Running) -> Result<Running, Cause> {
    let Some(cgroup) = running.cgroup.clone() else {
        return Ok(running);
    };
    let deadline = tokio::time::Instant::now() + JOIN;
    loop {
        if !matches!(running.child.try_wait(), Ok(None)) {
            return Ok(running);
        }
        let path = cgroup.clone();
        let members = tokio::task::spawn_blocking(move || tree::linux::members(&path))
            .await
            .map_err(|_| Cause::StartFailed)?;
        if members.is_ok_and(|members| members.contains(&running.leader)) {
            return Ok(running);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(Cause::ContainmentFailed(Containment::CgroupJoinFailed));
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
