use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::ids::{DeviceId, ProjectId};

use super::{Asker, Error, pin_in, store, unpin_in};
pub(crate) use store::source_placement::SourceNode;

pub(crate) struct SourceBuildPlacement {
    pub pinned: Option<SourceNode>,
    pub candidates: Vec<SourceNode>,
}

pub(crate) async fn source_placement(
    conn: &mut PgConnection,
    project: ProjectId,
) -> Result<SourceBuildPlacement, Error> {
    let nodes = store::source_placement::nodes(conn, project).await?;
    let pinned = nodes.iter().find(|node| node.pinned).cloned();
    let candidates = nodes
        .into_iter()
        .filter(|node| node.architecture.is_some())
        .collect();
    Ok(SourceBuildPlacement { pinned, candidates })
}

pub(crate) async fn set_source_pin(
    conn: &mut PgConnection,
    (project, device): (ProjectId, Option<DeviceId>),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<SourceBuildPlacement, Error> {
    if let Some(device) = device {
        let machine = store::machine_locked(conn, device).await?;
        let placement = source_placement(conn, project).await?;
        if machine.is_none()
            || !placement
                .candidates
                .iter()
                .any(|node| node.device == device)
            || !pin_in(conn, (project, device), asker, now).await?
        {
            return Err(Error::Invalid(
                "Choose an enrolled node that supports source builds.",
            ));
        }
    } else {
        unpin_in(conn, project, asker, now).await?;
    }
    source_placement(conn, project).await
}
