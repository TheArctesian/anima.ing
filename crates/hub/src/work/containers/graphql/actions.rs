//! Queue an exact Docker action through the work route.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "the async-graphql Enum derive generates async trait impls"
)]

use std::sync::Arc;

use async_graphql::{Context, Enum, ID, InputObject, Object, Result, SimpleObject};
use daemon_core::wire::{ContainerAction, ContainerVerb};

use crate::{
    api_errors::{ErrorCode, FieldError, coded, internal, invalid, pool, schema_dependency},
    clock::Clock,
    fleet::{
        access::{Authenticated, Identity, Scope},
        events::{Bus, ClientAddr},
    },
    ids::{DeviceId, JobId},
    work::jobs::{self, Job, Queue},
};

#[derive(Clone, Copy, Enum, PartialEq, Eq)]
pub(super) enum ContainerActionVerb {
    Start,
    Stop,
    Restart,
}

impl From<ContainerActionVerb> for ContainerVerb {
    fn from(verb: ContainerActionVerb) -> Self {
        match verb {
            ContainerActionVerb::Start => Self::Start,
            ContainerActionVerb::Stop => Self::Stop,
            ContainerActionVerb::Restart => Self::Restart,
        }
    }
}

#[derive(InputObject)]
pub(super) struct ContainerActionInput {
    device_id: ID,
    container_id: String,
    action: ContainerActionVerb,
}

#[derive(SimpleObject)]
pub(super) struct ContainerActionRequest {
    id: ID,
    state: String,
    reason: Option<String>,
}

impl From<Job> for ContainerActionRequest {
    fn from(job: Job) -> Self {
        Self {
            id: ID(job.id.to_string()),
            state: job.state.as_str().into(),
            reason: job.reason,
        }
    }
}

pub(super) async fn status(ctx: &Context<'_>, id: ID) -> Result<Option<ContainerActionRequest>> {
    let id: JobId = id
        .parse()
        .map_err(|_| bad("id", "Choose a valid action ID."))?;
    jobs::container_action(pool(ctx)?, id)
        .await
        .map(|action| action.map(Into::into))
        .map_err(internal)
}

#[derive(Default)]
pub struct Mutation;

#[Object(name = "ContainerActionsMutation")]
impl Mutation {
    /// Queue a managed Compose apply. Requires an operator login or browser session.
    #[graphql(guard = "Authenticated", complexity = "50 + child_complexity")]
    async fn queue_application_apply(
        &self,
        ctx: &Context<'_>,
        input: super::apply::ApplicationApplyInput,
    ) -> Result<super::apply::ApplicationApplyRequest> {
        super::apply::queue(ctx, input).await
    }

    /// Queue a start, stop or restart by full Docker ID on one named device.
    #[graphql(guard = "Authenticated")]
    async fn queue_container_action(
        &self,
        ctx: &Context<'_>,
        input: ContainerActionInput,
    ) -> Result<ContainerActionRequest> {
        let identity = schema_dependency::<Identity>(ctx)?;
        let operator = identity
            .operator()
            .filter(|_| identity.scope() == Scope::Write)
            .ok_or_else(|| {
                coded(
                    "A write credential is required to queue a container action.",
                    ErrorCode::Forbidden,
                )
            })?;
        let device: DeviceId = input
            .device_id
            .parse()
            .map_err(|_| bad("deviceId", "Choose a valid device."))?;
        let action = ContainerAction {
            container: input.container_id,
            action: input.action.into(),
        };
        if !action.full_id() {
            return Err(bad(
                "containerId",
                "Enter the full 64-digit Docker container ID.",
            ));
        }
        let queue = Queue {
            db: pool(ctx)?,
            bus: schema_dependency::<Bus>(ctx)?,
        };
        let client = *schema_dependency::<ClientAddr>(ctx)?;
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        jobs::enqueue_container_action(queue, (device, &action), (operator.clone(), client), now)
            .await
            .map(Into::into)
            .map_err(|error| match error {
                jobs::Error::Invalid(message) => coded(message, ErrorCode::Conflict),
                error => internal(error),
            })
    }
}

fn bad(field: &'static str, message: &str) -> async_graphql::Error {
    invalid(&[FieldError::new(field, message)])
}
