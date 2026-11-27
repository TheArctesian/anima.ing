use super::store;
use crate::{
    api_errors::{ErrorCode, coded, internal, pool},
    fleet::{
        access::{Authenticated, person_only},
        events::{ActorKind, AuditAction, AuditTarget, Severity, Source, Trail, TrailEntry},
    },
    ids::DeviceId,
};
use agent_threads::managed::workspace::Command;
use async_graphql::{Context, ID, InputObject, Json, Object, Result};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(InputObject)]
pub struct ManagedWorkspaceInput {
    request: ID,
    workspace: ID,
    action: Json<Value>,
}
#[derive(Default)]
pub struct Query;
#[Object(name = "ManagedWorkspaceQuery")]
impl Query {
    #[graphql(guard = "Authenticated")]
    async fn managed_node_workspaces(&self, ctx: &Context<'_>, device: ID) -> Result<Json<Value>> {
        let principal = person(ctx)?;
        let device = device.parse().map_err(|_| invalid())?;
        let result = store::inventory(pool(ctx)?, device, principal, Trail::from_context(ctx)?.now)
            .await
            .map_err(failed)?;
        #[cfg(unix)]
        let result = {
            let mut result = result;
            result["retainedSources"] = json!(
                super::super::handoffs::graphql::retained_sources(ctx, device, principal).await?
            );
            result
        };
        Ok(Json(result))
    }
    #[graphql(guard = "Authenticated")]
    async fn managed_workspace_request(
        &self,
        ctx: &Context<'_>,
        request: ID,
    ) -> Result<Option<Json<Value>>> {
        let principal = person(ctx)?;
        let request = request.parse().map_err(|_| invalid())?;
        let result = store::status(pool(ctx)?, request, principal)
            .await
            .map_err(failed)?;
        #[cfg(unix)]
        let result = if result.is_none() {
            super::super::handoffs::graphql::status(ctx, request, principal).await?
        } else {
            result
        };
        Ok(result.map(Json))
    }
}
#[derive(Default)]
pub struct Mutation;
#[Object(name = "ManagedWorkspaceMutation")]
impl Mutation {
    #[graphql(guard = "Authenticated")]
    async fn queue_managed_workspace_request(
        &self,
        ctx: &Context<'_>,
        device: ID,
        input: ManagedWorkspaceInput,
    ) -> Result<ID> {
        let principal = person(ctx)?;
        let device: DeviceId = device.parse().map_err(|_| invalid())?;
        let request = input.request.parse().map_err(|_| invalid())?;
        let workspace = input.workspace.parse().map_err(|_| invalid())?;
        #[cfg(unix)]
        if matches!(
            input.action.0["kind"].as_str(),
            Some("move" | "move-auto" | "withdraw-move" | "retire-source")
        ) {
            return super::super::handoffs::graphql::queue(
                ctx,
                (principal, device),
                (request, workspace),
                input.action.0,
            )
            .await;
        }
        let command = Command {
            request,
            workspace,
            principal,
            action: serde_json::from_value(input.action.0).map_err(|_| invalid())?,
        };
        let trail = Trail::from_context(ctx)?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        if store::enqueue(&mut tx, device, &command, trail.now)
            .await
            .map_err(failed)?
        {
            trail.write(&mut tx, TrailEntry {
                source: Source::Device, severity: Severity::Info, device: None,
                message: "Managed workspace lifecycle requested.".to_owned(),
                action: AuditAction::Update, target: AuditTarget::Device(device),
                change: json!({"workspaceRequest":command.request,"workspace":command.workspace,"action":command.action}),
            }).await.map_err(internal)?;
        }
        tx.commit().await.map_err(internal)?;
        Ok(ID(command.request.to_string()))
    }
}
fn person(ctx: &Context<'_>) -> Result<Uuid> {
    let identity = person_only(ctx)?;
    let operator = identity.operator().ok_or_else(invalid)?;
    if operator.actor.kind() != ActorKind::User {
        return Err(coded(
            "Only the signed-in person may manage workspaces.",
            ErrorCode::Forbidden,
        ));
    }
    Ok(operator.id.into())
}
fn invalid() -> async_graphql::Error {
    coded(
        "Invalid workspace lifecycle request.",
        ErrorCode::Validation,
    )
}
fn failed(error: super::Error) -> async_graphql::Error {
    match error {
        super::Error::Refused(message) => coded(message, ErrorCode::Conflict),
        other => internal(other),
    }
}
