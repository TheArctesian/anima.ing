use super::{browser_sessions, previews, store};
use crate::{
    api_errors::{ErrorCode, coded, internal, pool},
    fleet::{
        access::{Authenticated, person_only},
        events::{ActorKind, AuditAction, AuditTarget, Severity, Source, Trail, TrailEntry},
    },
    ids::DeviceId,
};
use agent_threads::managed::Command;
use async_graphql::{Context, ID, InputObject, Json, Object, Result};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(InputObject)]
pub struct ManagedCodeInput {
    request: ID,
    workspace: ID,
    generation: String,
    action: Json<Value>,
}

#[derive(InputObject)]
pub struct ManagedCodePreviewInput {
    endpoint: ID,
    workspace: ID,
    generation: String,
    port: i32,
}

#[derive(InputObject)]
pub struct ManagedCodeWorkspaceInput {
    workspace: ID,
    enabled: bool,
}

#[derive(Default)]
pub struct Query;
#[Object(name = "ManagedCodeQuery")]
impl Query {
    #[graphql(guard = "Authenticated")]
    async fn managed_code_previews(
        &self,
        ctx: &Context<'_>,
        device: ID,
        workspace: ID,
    ) -> Result<Vec<Json<Value>>> {
        previews::list(
            pool(ctx)?,
            &previews::Scope {
                device: device.parse().map_err(|_| invalid())?,
                workspace: workspace.parse().map_err(|_| invalid())?,
                principal: person(ctx)?,
            },
            Trail::from_context(ctx)?.now,
        )
        .await
        .map(|rows| rows.into_iter().map(Json).collect())
        .map_err(failed)
    }

    #[graphql(guard = "Authenticated")]
    async fn managed_code_workspaces(
        &self,
        ctx: &Context<'_>,
        device: ID,
    ) -> Result<Vec<Json<Value>>> {
        store::workspaces(
            pool(ctx)?,
            device.parse().map_err(|_| invalid())?,
            Trail::from_context(ctx)?.now,
            person(ctx).ok(),
        )
        .await
        .map(|rows| rows.into_iter().map(Json).collect())
        .map_err(failed)
    }
    #[graphql(guard = "Authenticated")]
    async fn managed_code_command(
        &self,
        ctx: &Context<'_>,
        request: ID,
    ) -> Result<Option<Json<Value>>> {
        let principal = person(ctx)?;
        store::status(
            pool(ctx)?,
            request.parse().map_err(|_| invalid())?,
            principal,
        )
        .await
        .map(|row| row.map(Json))
        .map_err(failed)
    }
}

#[derive(Default)]
pub struct Mutation;
#[Object(name = "ManagedCodeMutation")]
impl Mutation {
    #[graphql(guard = "Authenticated")]
    async fn managed_code_preview_handoff(
        &self,
        ctx: &Context<'_>,
        endpoint: ID,
    ) -> Result<Json<Value>> {
        let principal = person(ctx)?;
        let endpoint: Uuid = endpoint.parse().map_err(|_| invalid())?;
        let config =
            crate::api_errors::schema_dependency::<std::sync::Arc<crate::config::Config>>(ctx)?;
        let origins = config.preview_origins.as_ref().ok_or_else(|| {
            coded(
                "Browser previews require a dedicated preview origin.",
                ErrorCode::Validation,
            )
        })?;
        let token = browser_sessions::issue(
            pool(ctx)?,
            endpoint,
            principal,
            Trail::from_context(ctx)?.now,
        )
        .await
        .map_err(failed)?;
        Ok(Json(
            json!({"action":format!("{}/.anima/handoff", origins.origin(endpoint)),"token":token,"method":"POST"}),
        ))
    }

    #[graphql(guard = "Authenticated")]
    async fn open_managed_code_preview(
        &self,
        ctx: &Context<'_>,
        device: ID,
        input: ManagedCodePreviewInput,
    ) -> Result<Json<Value>> {
        let principal = person(ctx)?;
        let config =
            crate::api_errors::schema_dependency::<std::sync::Arc<crate::config::Config>>(ctx)?;
        if config.preview_origins.is_none() {
            return Err(coded(
                "Browser previews require a dedicated preview origin.",
                ErrorCode::Validation,
            ));
        }
        let device: DeviceId = device.parse().map_err(|_| invalid())?;
        let endpoint: Uuid = input.endpoint.parse().map_err(|_| invalid())?;
        let workspace: Uuid = input.workspace.parse().map_err(|_| invalid())?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        let value = previews::issue(
            &mut tx,
            &previews::Request {
                scope: previews::Scope {
                    device,
                    workspace,
                    principal,
                },
                endpoint,
                generation: input.generation.parse().map_err(|_| invalid())?,
                port: input.port.try_into().map_err(|_| invalid())?,
            },
            Trail::from_context(ctx)?.now,
        )
        .await
        .map_err(failed)?;
        audit(
            ctx,
            &mut tx,
            device,
            json!({"preview":endpoint,"workspace":workspace,"port":input.port,"action":"open"}),
        )
        .await?;
        tx.commit().await.map_err(internal)?;
        Ok(Json(value))
    }
    #[graphql(guard = "Authenticated")]
    async fn close_managed_code_preview(&self, ctx: &Context<'_>, endpoint: ID) -> Result<bool> {
        let principal = person(ctx)?;
        let endpoint: Uuid = endpoint.parse().map_err(|_| invalid())?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        let device = previews::revoke(&mut tx, endpoint, principal)
            .await
            .map_err(failed)?;
        audit(
            ctx,
            &mut tx,
            device,
            json!({"preview":endpoint,"action":"close"}),
        )
        .await?;
        tx.commit().await.map_err(internal)?;
        Ok(true)
    }

    #[graphql(guard = "Authenticated")]
    async fn set_managed_code_workspace_enabled(
        &self,
        ctx: &Context<'_>,
        device: ID,
        input: ManagedCodeWorkspaceInput,
    ) -> Result<bool> {
        let ManagedCodeWorkspaceInput { workspace, enabled } = input;
        person(ctx)?;
        let device: DeviceId = device.parse().map_err(|_| invalid())?;
        let workspace: Uuid = workspace.parse().map_err(|_| invalid())?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        store::enabled(&mut tx, device, workspace, enabled)
            .await
            .map_err(failed)?;
        audit(
            ctx,
            &mut tx,
            device,
            json!({"managedWorkspace":workspace,"enabled":enabled}),
        )
        .await?;
        tx.commit().await.map_err(internal)?;
        Ok(enabled)
    }
    #[graphql(guard = "Authenticated")]
    async fn queue_managed_code_command(
        &self,
        ctx: &Context<'_>,
        device: ID,
        input: ManagedCodeInput,
    ) -> Result<ID> {
        let principal = person(ctx)?;
        let device: DeviceId = device.parse().map_err(|_| invalid())?;
        let command = Command {
            request: input.request.parse().map_err(|_| invalid())?,
            workspace: input.workspace.parse().map_err(|_| invalid())?,
            generation: input.generation.parse().map_err(|_| invalid())?,
            principal,
            action: serde_json::from_value(input.action.0).map_err(|_| invalid())?,
        };
        let trail = Trail::from_context(ctx)?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        if store::enqueue(&mut tx, device, &command, trail.now)
            .await
            .map_err(failed)?
        {
            audit(
                ctx,
                &mut tx,
                device,
                json!({"managedCommand":command.request,
                "workspace":command.workspace,"generation":command.generation}),
            )
            .await?;
        }
        tx.commit().await.map_err(internal)?;
        Ok(ID(command.request.to_string()))
    }
}

pub(super) fn person(ctx: &Context<'_>) -> Result<Uuid> {
    let identity = person_only(ctx)?;
    let operator = identity.operator().ok_or_else(invalid)?;
    if operator.actor.kind() != ActorKind::User {
        return Err(coded(
            "Only the signed-in person may control managed Code.",
            ErrorCode::Forbidden,
        ));
    }
    Ok(operator.id.into())
}
fn invalid() -> async_graphql::Error {
    coded("Invalid managed Code request.", ErrorCode::Validation)
}
fn failed(error: super::Error) -> async_graphql::Error {
    match error {
        super::Error::Refused(message) => coded(message, ErrorCode::Conflict),
        other => internal(other),
    }
}
async fn audit(
    ctx: &Context<'_>,
    tx: &mut sqlx::PgConnection,
    device: DeviceId,
    change: Value,
) -> Result<()> {
    Trail::from_context(ctx)?
        .write(
            tx,
            TrailEntry {
                source: Source::Device,
                severity: Severity::Info,
                device: None,
                message: "Managed Code authorization or delivery requested.".to_owned(),
                action: AuditAction::Update,
                target: AuditTarget::Device(device),
                change,
            },
        )
        .await
        .map_err(internal)
}
