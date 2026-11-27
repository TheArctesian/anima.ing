//! A signed-in operator chooses the project's build-device pin.
use std::sync::Arc;

use async_graphql::{Context, ID, Object, Result, SimpleObject};

use super::{
    super::{on, store::reads},
    builds::SourceBuildTarget,
};
use crate::{
    api_errors::{ErrorCode, FieldError, coded, internal, invalid, schema_dependency},
    clock::Clock,
    fleet::{
        access::{Authenticated, operator_only},
        events::{ActorKind, ClientAddr},
    },
    ids::{DeviceId, ProjectId},
    work::jobs::{self, SourceBuildPlacement, SourceNode},
};

#[derive(SimpleObject)]
struct ProjectBuildNode {
    id: ID,
    name: String,
    target: Option<SourceBuildTarget>,
    private_checkout: bool,
}

impl From<SourceNode> for ProjectBuildNode {
    fn from(node: SourceNode) -> Self {
        let target = match node.architecture.as_deref() {
            Some("x86_64") => Some(SourceBuildTarget::X86_64),
            Some("aarch64") => Some(SourceBuildTarget::Aarch64),
            _ => None,
        };
        Self {
            id: ID(node.device.to_string()),
            name: node.name,
            target,
            private_checkout: node.private_checkout,
        }
    }
}

#[derive(SimpleObject)]
struct ProjectBuildPlacement {
    pinned: Option<ProjectBuildNode>,
    candidates: Vec<ProjectBuildNode>,
}

impl From<SourceBuildPlacement> for ProjectBuildPlacement {
    fn from(placement: SourceBuildPlacement) -> Self {
        Self {
            pinned: placement.pinned.map(Into::into),
            candidates: placement.candidates.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Default)]
pub(super) struct Query;

#[Object(name = "ProjectBuildPlacementQuery")]
impl Query {
    /// Read the current build pin and candidate nodes. Login or browser token only (D-23, R-100).
    #[graphql(guard = "Authenticated")]
    async fn project_build_placement(
        &self,
        ctx: &Context<'_>,
        project: ID,
    ) -> Result<ProjectBuildPlacement> {
        operator_only(ctx, "projectBuildPlacement")?;
        let db = on(ctx).await?;
        let project = parse::<ProjectId>(&project, "project")?;
        if reads::by_id(db, project).await.map_err(internal)?.is_none() {
            return Err(missing());
        }
        let mut conn = db.acquire().await.map_err(internal)?;
        jobs::source_placement(&mut conn, project)
            .await
            .map(Into::into)
            .map_err(internal)
    }
}

#[derive(Default)]
pub(super) struct Mutation;

#[Object(name = "ProjectBuildPlacementMutation")]
impl Mutation {
    /// Select a node for this project's builds, or clear the pin with a null device.
    /// Login or browser token only (D-23, R-100).
    #[graphql(guard = "Authenticated")]
    async fn set_project_build_pin(
        &self,
        ctx: &Context<'_>,
        project: ID,
        device: Option<ID>,
    ) -> Result<ProjectBuildPlacement> {
        let identity = operator_only(ctx, "setProjectBuildPin")?;
        let actor = identity
            .operator()
            .filter(|actor| actor.actor.kind() == ActorKind::User)
            .ok_or_else(|| {
                coded(
                    "A signed-in operator must set the build pin.",
                    ErrorCode::Forbidden,
                )
            })?;
        let db = on(ctx).await?;
        let project = parse::<ProjectId>(&project, "project")?;
        let device = device
            .as_ref()
            .map(|id| parse::<DeviceId>(id, "device"))
            .transpose()?;
        let client = *schema_dependency::<ClientAddr>(ctx)?;
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        let mut tx = db.begin().await.map_err(internal)?;
        if !reads::lock_project(&mut tx, project)
            .await
            .map_err(internal)?
        {
            return Err(missing());
        }
        let updated =
            jobs::set_source_pin(&mut tx, (project, device), (actor.clone(), client), now)
                .await
                .map_err(|error| match error {
                    jobs::Error::Invalid(message) => coded(message, ErrorCode::Conflict),
                    other => internal(other),
                })?;
        tx.commit().await.map_err(internal)?;
        Ok(updated.into())
    }
}

fn missing() -> async_graphql::Error {
    coded("No project has this ID.", ErrorCode::NotFound)
}

fn parse<T: std::str::FromStr>(id: &ID, field: &'static str) -> Result<T> {
    id.parse()
        .map_err(|_| invalid(&[FieldError::new(field, "Choose a valid ID.")]))
}
