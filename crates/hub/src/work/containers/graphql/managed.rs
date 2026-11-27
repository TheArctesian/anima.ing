//! Correlate Docker inventory with completed applies for the same device.
#[cfg(test)]
mod tests;

use async_graphql::Result;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{api_errors::internal, ids::DeviceId, work::jobs};

use super::Container;

// The state directory of the NixOS applications service (os/modules/containers.nix).
const ROOT: &str = "/persist/anima-applications";

pub(super) async fn mark(
    db: &PgPool,
    device: DeviceId,
    containers: &mut [Container],
) -> Result<()> {
    let observed: Vec<_> = containers.iter().map(candidate).collect();
    let mut ids: Vec<_> = observed.iter().flatten().copied().collect();
    if ids.is_empty() {
        return Ok(());
    }
    ids.sort_unstable();
    ids.dedup();
    let installed = jobs::installed_applications(db, device, &ids)
        .await
        .map_err(internal)?;
    for (container, id) in containers.iter_mut().zip(observed) {
        container.managed_application_id = id.filter(|id| installed.contains(id));
    }
    Ok(())
}

fn candidate(container: &Container) -> Option<Uuid> {
    let project = container
        .compose_project
        .as_deref()?
        .strip_prefix("anima-")?;
    let id = Uuid::try_parse(project).ok()?;
    if project != id.simple().to_string() || container.compose_service.is_none() {
        return None;
    }
    let folder = format!("{ROOT}/{id}/files");
    let exact = container.compose_working_dir.as_deref() == Some(folder.as_str())
        && container.compose_files == [format!("{folder}/final-form.json")];
    exact.then_some(id)
}
