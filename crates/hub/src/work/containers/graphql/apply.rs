//! The operator-facing entry for a managed Compose application.
use std::sync::Arc;

use async_graphql::{Context, ID, InputObject, Result, SimpleObject};
use daemon_core::wire::{ApplicationApply, ApplicationHealth};
use uuid::Uuid;

use crate::{
    api_errors::{ErrorCode, FieldError, coded, internal, invalid, pool, schema_dependency},
    clock::Clock,
    fleet::{
        access::operator_only,
        credentials::{Secret, Vault},
        events::{ActorKind, Bus, ClientAddr},
    },
    ids::{DeviceId, JobId},
    work::jobs::{self, ApplicationFiles, Job, Queue},
};

#[derive(InputObject)]
pub(super) struct ApplicationApplyInput {
    device_id: ID,
    /// Use the application ID from a previous apply to update that application.
    application_id: Option<ID>,
    compose_file: String,
    environment_file: Option<String>,
    #[graphql(default = true)]
    pull: bool,
    #[graphql(default = true)]
    keep_previous: bool,
}

#[derive(SimpleObject)]
pub(super) struct ApplicationApplyRequest {
    id: ID,
    application_id: ID,
    device_id: ID,
    state: String,
    reason: Option<String>,
    health: Option<String>,
}

#[derive(SimpleObject)]
pub(super) struct ApplicationApplyTarget {
    id: ID,
    name: String,
}

impl TryFrom<Job> for ApplicationApplyRequest {
    type Error = jobs::Error;

    fn try_from(job: Job) -> std::result::Result<Self, Self::Error> {
        let parameters: ApplicationApply =
            serde_json::from_value(job.asked.parameters).map_err(|_| jobs::Error::Unreadable)?;
        let device = job.asked.machine.ok_or(jobs::Error::Unreadable)?;
        let health = job.result.and_then(|result| result.health).map(health_name);
        Ok(Self {
            id: ID(job.id.to_string()),
            application_id: ID(parameters.application.to_string()),
            device_id: ID(device.to_string()),
            state: job.state.as_str().to_owned(),
            reason: job.reason,
            health,
        })
    }
}

pub(super) async fn queue(
    ctx: &Context<'_>,
    input: ApplicationApplyInput,
) -> Result<ApplicationApplyRequest> {
    let files = ApplicationFiles {
        compose: Secret::from(input.compose_file),
        environment: input.environment_file.map(Secret::from),
    };
    let identity = operator_only(ctx, "queueApplicationApply")?;
    let operator = identity
        .operator()
        .filter(|operator| operator.actor.kind() == ActorKind::User)
        .ok_or_else(|| coded("An operator session is required.", ErrorCode::Forbidden))?;
    let device = input
        .device_id
        .parse::<DeviceId>()
        .map_err(|_| bad("deviceId", "Choose a valid device."))?;
    if !input.pull && input.application_id.is_none() {
        return Err(bad(
            "applicationId",
            "An existing application is required to reuse images.",
        ));
    }
    let application = input.application_id.map_or_else(
        || Ok(Uuid::now_v7()),
        |id| {
            id.parse::<Uuid>()
                .map_err(|_| bad("applicationId", "Enter a valid application ID."))
        },
    )?;
    let queue = Queue {
        db: pool(ctx)?,
        bus: schema_dependency::<Bus>(ctx)?,
    };
    let vault = schema_dependency::<Vault>(ctx)?;
    let client = *schema_dependency::<ClientAddr>(ctx)?;
    let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
    jobs::enqueue_application_apply(
        (queue, vault),
        (application, device, input.pull, input.keep_previous),
        files,
        ((operator.clone(), client), now),
    )
    .await
    .map_err(|error| match error {
        jobs::Error::Invalid(message) => coded(message, ErrorCode::Conflict),
        error => internal(error),
    })?
    .try_into()
    .map_err(internal)
}

pub(super) async fn status(ctx: &Context<'_>, id: ID) -> Result<Option<ApplicationApplyRequest>> {
    let id = id
        .parse::<JobId>()
        .map_err(|_| bad("id", "Enter a valid apply ID."))?;
    jobs::application_apply(pool(ctx)?, id)
        .await
        .map_err(internal)?
        .map(TryInto::try_into)
        .transpose()
        .map_err(internal)
}

pub(super) async fn targets(ctx: &Context<'_>) -> Result<Vec<ApplicationApplyTarget>> {
    let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
    let targets = jobs::application_apply_targets(pool(ctx)?, now)
        .await
        .map_err(internal)?;
    Ok(targets
        .into_iter()
        .map(|(id, name)| ApplicationApplyTarget {
            id: ID(id.to_string()),
            name,
        })
        .collect())
}

fn health_name(health: ApplicationHealth) -> String {
    match health {
        ApplicationHealth::Healthy => "healthy",
        ApplicationHealth::Unhealthy => "unhealthy",
        ApplicationHealth::NoCheck => "no_check",
        ApplicationHealth::NotWaited => "not_waited",
        ApplicationHealth::Unknown => "unknown",
    }
    .to_owned()
}

fn bad(field: &'static str, message: &str) -> async_graphql::Error {
    invalid(&[FieldError::new(field, message)])
}
