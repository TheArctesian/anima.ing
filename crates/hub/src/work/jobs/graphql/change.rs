use async_graphql::{Context, ID, Object, Result};
use serde_json::json;
use sqlx::PgConnection;

use super::super::{set_device_work as set_work, store};
use super::{bad, stop};
use crate::{
    api_errors::{ErrorCode, begin, coded, commit, internal},
    fleet::{
        access::{Authenticated, person_only},
        devices::{self, MetricsSource},
        events::{AuditAction, AuditTarget, Severity, Source, Trail, TrailEntry},
    },
    ids::{DeviceId, JobId, ProjectId},
};

#[derive(Default)]
pub struct Mutation;

#[Object(name = "JobsMutation")]
impl Mutation {
    #[graphql(guard = "Authenticated")]
    async fn stop_job(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        person_only(ctx)?;
        stop(ctx, id.parse::<JobId>().map_err(|_| bad("id"))?).await
    }

    #[graphql(guard = "Authenticated")]
    async fn set_device_work(&self, ctx: &Context<'_>, device_id: ID, on: bool) -> Result<bool> {
        person_only(ctx)?;
        let id = device(&device_id)?;
        let audit = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        eligible_device(&mut tx, id).await?;
        set_work(&mut tx, id, on, audit.now)
            .await
            .map_err(internal)?;
        audit
            .write(
                &mut tx,
                changed(AuditTarget::Device(id), json!({"workOn": on})),
            )
            .await
            .map_err(internal)?;
        commit(tx).await?;
        Ok(true)
    }

    #[graphql(guard = "Authenticated")]
    async fn set_project_work_devices(
        &self,
        ctx: &Context<'_>,
        project_id: ID,
        device_ids: Vec<ID>,
    ) -> Result<bool> {
        person_only(ctx)?;
        let project = project(&project_id)?;
        let devices = permitted(&device_ids)?;
        let audit = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        lock_project(&mut tx, project).await?;
        for id in &devices {
            eligible_device(&mut tx, *id).await?;
        }
        store::admin::replace_devices(&mut tx, project, &devices)
            .await
            .map_err(internal)?;
        audit
            .write(
                &mut tx,
                changed(
                    AuditTarget::Project(project),
                    json!({"workDevices": devices}),
                ),
            )
            .await
            .map_err(internal)?;
        commit(tx).await?;
        Ok(true)
    }

    #[graphql(guard = "Authenticated")]
    async fn set_project_work_machine(
        &self,
        ctx: &Context<'_>,
        project_id: ID,
        device_id: Option<ID>,
    ) -> Result<bool> {
        person_only(ctx)?;
        let project = project(&project_id)?;
        let device = device_id.as_ref().map(device).transpose()?;
        let audit = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        lock_project(&mut tx, project).await?;
        if let Some(id) = device {
            eligible_device(&mut tx, id).await?;
            store::pin(&mut tx, project, id).await.map_err(internal)?;
        } else {
            store::unpin(&mut tx, project).await.map_err(internal)?;
        }
        audit
            .write(
                &mut tx,
                changed(
                    AuditTarget::Project(project),
                    json!({"workMachine": device}),
                ),
            )
            .await
            .map_err(internal)?;
        commit(tx).await?;
        Ok(true)
    }

    #[graphql(guard = "Authenticated")]
    async fn set_device_work_excluded(
        &self,
        ctx: &Context<'_>,
        device_id: ID,
        excluded: bool,
    ) -> Result<bool> {
        person_only(ctx)?;
        let id = device(&device_id)?;
        let audit = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        eligible_device(&mut tx, id).await?;
        if !store::set_excluded(&mut tx, id, excluded)
            .await
            .map_err(internal)?
        {
            return Ok(false);
        }
        audit
            .write(
                &mut tx,
                changed(AuditTarget::Device(id), json!({"workExcluded": excluded})),
            )
            .await
            .map_err(internal)?;
        commit(tx).await?;
        Ok(true)
    }
}

fn device(id: &ID) -> Result<DeviceId> {
    id.parse::<DeviceId>().map_err(|_| bad("deviceId"))
}

fn project(id: &ID) -> Result<ProjectId> {
    id.parse::<ProjectId>().map_err(|_| bad("projectId"))
}

fn permitted(ids: &[ID]) -> Result<Vec<DeviceId>> {
    if ids.len() > 100 {
        return Err(bad("deviceIds"));
    }
    let mut devices = ids.iter().map(device).collect::<Result<Vec<_>>>()?;
    devices.sort_by_key(|id| uuid::Uuid::from(*id));
    devices.dedup();
    if devices.len() != ids.len() {
        return Err(bad("deviceIds"));
    }
    Ok(devices)
}

async fn eligible_device(conn: &mut PgConnection, id: DeviceId) -> Result<()> {
    match devices::lock_metrics_source(conn, id).await {
        Ok(MetricsSource::Daemon) => Ok(()),
        Ok(_) | Err(devices::Error::NotFound(_)) => {
            Err(coded("No such enrolled device.", ErrorCode::NotFound))
        }
        Err(error) => Err(internal(error)),
    }
}

async fn lock_project(conn: &mut PgConnection, id: ProjectId) -> Result<()> {
    if store::admin::lock_project(conn, id)
        .await
        .map_err(internal)?
    {
        Ok(())
    } else {
        Err(coded("No such project.", ErrorCode::NotFound))
    }
}

fn changed(target: AuditTarget, change: serde_json::Value) -> TrailEntry {
    TrailEntry {
        source: Source::Hub,
        severity: Severity::Info,
        device: None,
        message: "Changed work placement settings.".to_owned(),
        action: AuditAction::Update,
        target,
        change,
    }
}
