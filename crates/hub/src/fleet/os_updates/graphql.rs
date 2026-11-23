use async_graphql::{Context, ID, InputObject, Json, Object, Result};
use serde_json::{Value, json};

use super::{OsUpdateAction, run};
use crate::{
    api_errors::{ErrorCode, coded, internal, schema_dependency},
    fleet::{
        access::{Authenticated, person_only},
        credentials::Vault,
        devices,
        events::{AuditAction, AuditTarget, Severity, Source, Trail, TrailEntry},
        ssh,
        targets::AddressPolicy,
    },
    ids::DeviceId,
};
use sqlx::PgPool;

#[derive(Default)]
pub struct Query;

#[Object(name = "OsUpdatesQuery")]
impl Query {
    #[graphql(guard = "Authenticated")]
    async fn device_os_update(&self, ctx: &Context<'_>, device_id: ID) -> Result<Json<Value>> {
        let (_, mut link) = connection(ctx, device_id).await?;
        run(&mut link, None)
            .await
            .map(Json)
            .map_err(|error| refusal(&error))
    }
}

#[derive(InputObject)]
pub struct OsUpdateRequest {
    action: OsUpdateAction,
    #[graphql(default)]
    confirmed: bool,
}

#[derive(Default)]
pub struct Mutation;

#[Object(name = "OsUpdatesMutation")]
impl Mutation {
    #[graphql(guard = "Authenticated")]
    async fn request_device_os_update(
        &self,
        ctx: &Context<'_>,
        device_id: ID,
        input: OsUpdateRequest,
    ) -> Result<bool> {
        person_only(ctx)?;
        let OsUpdateRequest { action, confirmed } = input;
        if action.restarts() && !confirmed {
            return Err(coded(
                "Confirm restarting the device first.",
                ErrorCode::Validation,
            ));
        }
        let (device, mut link) = connection(ctx, device_id).await?;
        let db = schema_dependency::<PgPool>(ctx)?;
        let mut tx = db.begin().await.map_err(internal)?;
        Trail::from_context(ctx)?
            .write(
                &mut tx,
                TrailEntry {
                    source: Source::Device,
                    severity: Severity::Info,
                    device: Some((device.id, device.name.clone())),
                    message: format!("OS update {action:?} requested for {}.", device.name),
                    action: AuditAction::Update,
                    target: AuditTarget::Device(device.id),
                    change: json!({"osUpdate":format!("{action:?}"),"confirmed":confirmed}),
                },
            )
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        let result = run(&mut link, Some(action))
            .await
            .map_err(|error| refusal(&error))?;
        Ok(result["requested"] == true)
    }
}

async fn connection(ctx: &Context<'_>, id: ID) -> Result<(devices::Device, ssh::SshLink)> {
    let id = id
        .parse::<DeviceId>()
        .map_err(|_| coded("Invalid device id.", ErrorCode::Validation))?;
    let db = schema_dependency::<PgPool>(ctx)?;
    let device = devices::get(db, id).await.map_err(|error| match error {
        devices::Error::NotFound(_) => coded("No such device.", ErrorCode::NotFound),
        other => internal(other),
    })?;
    if !device
        .host
        .parse::<std::net::Ipv6Addr>()
        .is_ok_and(|ip| ip.segments()[..3] == [0xfd41, 0x4e49, 0x4d41])
    {
        return Err(coded(
            "OS updates require the device's private Anima connection.",
            ErrorCode::Conflict,
        ));
    }
    let link = ssh::link(
        db,
        schema_dependency::<Vault>(ctx)?,
        schema_dependency::<AddressPolicy>(ctx)?,
        &device,
        Trail::from_context(ctx)?.now,
    )
    .await
    .map_err(|error| refusal(&error.into()))?;
    Ok((device, link))
}

fn refusal(error: &super::Error) -> async_graphql::Error {
    coded(error.to_string(), ErrorCode::Unavailable)
}
