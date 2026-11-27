//! Project pins alongside the nodes' last source-build capabilities.
use daemon_core::wire::SOURCE_IMAGE_PRIVATE_CHECKOUT;
use sqlx::PgConnection;

use crate::ids::{DeviceId, ProjectId};

use super::Error;

#[derive(Clone, sqlx::FromRow)]
pub(crate) struct SourceNode {
    pub device: DeviceId,
    pub name: String,
    pub architecture: Option<String>,
    pub private_checkout: bool,
    pub pinned: bool,
}

pub(in crate::work::jobs) async fn nodes(
    conn: &mut PgConnection,
    project: ProjectId,
) -> Result<Vec<SourceNode>, Error> {
    Ok(sqlx::query_as(
        "WITH offered AS (
           SELECT d.id AS device, d.name, m.architecture, m.build_recipes,
             p.device_id IS NOT NULL AS pinned,
             COALESCE(d.metrics_source='daemon' AND m.os='linux'
               AND m.architecture IN ('aarch64','x86_64') AND 'build'=ANY(m.types)
               AND 'container'=ANY(m.kinds) AND m.build_recipes ? 'source_image_v1',false) AS eligible
           FROM devices.devices d
           LEFT JOIN jobs.machines m ON m.device_id=d.id
           LEFT JOIN jobs.project_machines p ON p.project_id=$1 AND p.device_id=d.id
         )
         SELECT device,name,pinned,CASE WHEN eligible THEN architecture END AS architecture,
           eligible AND COALESCE(build_recipes ? $2,false) AS private_checkout
         FROM offered WHERE pinned OR eligible ORDER BY lower(name),device",
    )
    .bind(project).bind(SOURCE_IMAGE_PRIVATE_CHECKOUT).fetch_all(conn).await?)
}
