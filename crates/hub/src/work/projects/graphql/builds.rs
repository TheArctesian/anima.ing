//! Project-bound source build submission and status.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "the async-graphql Enum derive generates async trait impls"
)]

use std::sync::Arc;

use async_graphql::{Context, Enum, ID, InputObject, Object, Result, SimpleObject};
use daemon_core::wire::{Limits, REPOSITORY_TOKEN, WorkKind, WorkRepository, WorkType};
use serde_json::json;
use source_exchange::{CommitSha, RepositoryName};

use super::super::{
    forge::{self, BuildVisibility, Target},
    on,
};
use crate::{
    api_errors::{ErrorCode, FieldError, coded, internal, invalid, pool, schema_dependency},
    clock::Clock,
    fleet::{
        access::{Authenticated, operator_only},
        events::{ActorKind, Bus, ClientAddr},
    },
    ids::JobId,
    work::jobs::{self, CheckoutSource, Job, NewJob, Queue},
};

const MIB: u64 = 1024 * 1024;

#[derive(Clone, Copy, Enum, PartialEq, Eq)]
pub(super) enum SourceBuildTarget {
    #[graphql(name = "LINUX_X86_64")]
    X86_64,
    #[graphql(name = "LINUX_AARCH64")]
    Aarch64,
}

#[derive(Clone, Copy, Default, Enum, PartialEq, Eq)]
enum SourceBuildStrategy {
    #[default]
    Auto,
    Dockerfile,
    Railpack,
}

#[derive(InputObject)]
pub(super) struct SourceBuildInput {
    request: Option<ID>,
    repository: ID,
    commit: String,
    target: SourceBuildTarget,
    #[graphql(default)]
    strategy: SourceBuildStrategy,
    memory_mib: Option<u32>,
}

#[derive(SimpleObject)]
pub(super) struct SourceBuild {
    id: ID,
    state: String,
    attempt: u32,
    reason: Option<String>,
}

impl From<Job> for SourceBuild {
    fn from(job: Job) -> Self {
        Self {
            id: ID(job.id.to_string()),
            state: job.state.as_str().into(),
            attempt: job.attempt,
            reason: job.reason,
        }
    }
}

#[derive(Default)]
pub(super) struct Query;

#[Object(name = "SourceBuildQuery")]
impl Query {
    /// Lookup by build ID. Login or browser token only (D-23, R-100).
    #[graphql(guard = "Authenticated")]
    async fn source_build(&self, ctx: &Context<'_>, id: ID) -> Result<Option<SourceBuild>> {
        operator_only(ctx, "sourceBuild")?;
        let id: JobId = id
            .parse()
            .map_err(|_| bad("id", "Choose a valid build ID."))?;
        jobs::source_job(on(ctx).await?, id)
            .await
            .map(|job| job.map(Into::into))
            .map_err(internal)
    }
}

#[derive(Default)]
pub(super) struct Mutation;

#[Object(name = "SourceBuildMutation")]
impl Mutation {
    /// Queue an immutable project revision. Login or browser token only (D-23, R-100).
    #[graphql(guard = "Authenticated")]
    async fn queue_source_build(
        &self,
        ctx: &Context<'_>,
        input: SourceBuildInput,
    ) -> Result<SourceBuild> {
        let identity = operator_only(ctx, "queueSourceBuild")?;
        let actor = identity
            .operator()
            .filter(|actor| actor.actor.kind() == ActorKind::User)
            .ok_or_else(|| {
                coded(
                    "A signed-in operator must queue this build.",
                    ErrorCode::Forbidden,
                )
            })?;
        let target = forge::target(ctx, &input.repository).await?;
        let wanted = request(&target, &input)?;
        let queue = Queue {
            db: pool(ctx)?,
            bus: schema_dependency::<Bus>(ctx)?,
        };
        let client = *schema_dependency::<ClientAddr>(ctx)?;
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        let asker = (actor.clone(), client);
        let result =
            if input.request.is_some() || target.build_visibility == BuildVisibility::Private {
                let checkout = checkout_source(&target)?;
                let request = input.request.map_or_else(
                    || Ok(uuid::Uuid::now_v7()),
                    |request| {
                        let request = request
                            .parse::<uuid::Uuid>()
                            .ok()
                            .filter(|id| !id.is_nil())
                            .ok_or_else(|| {
                                bad("request", "Choose a nonempty build request identity.")
                            })?;
                        Ok::<_, async_graphql::Error>(request)
                    },
                )?;
                let source = jobs::SourceSubmission {
                    request,
                    repository: target.repository,
                    checkout,
                };
                jobs::enqueue_source(queue, (wanted, source), asker, now).await
            } else {
                jobs::enqueue(queue, wanted, asker, now).await
            };
        result.map(Into::into).map_err(|error| match error {
            jobs::Error::NoFit(_) | jobs::Error::Invalid(_) => {
                coded(error.to_string(), ErrorCode::Conflict)
            }
            other => internal(other),
        })
    }
}

fn request(target: &Target, input: &SourceBuildInput) -> Result<NewJob> {
    if target.build_visibility == BuildVisibility::Archived {
        return Err(coded(
            "Source builds require an active repository.",
            ErrorCode::Conflict,
        ));
    }
    let name = RepositoryName::parse(&target.name).map_err(internal)?;
    let commit = CommitSha::parse(&input.commit)
        .map_err(|_| bad("commit", "Use a full 40-digit hexadecimal commit."))?;
    let memory = input.memory_mib.unwrap_or(2048);
    if !(512..=2048).contains(&memory) {
        return Err(bad("memoryMib", "Choose between 512 and 2048 MiB."));
    }
    let target_name = match input.target {
        SourceBuildTarget::X86_64 => "linux-x86_64",
        SourceBuildTarget::Aarch64 => "linux-aarch64",
    };
    let strategy = match input.strategy {
        SourceBuildStrategy::Auto => "auto",
        SourceBuildStrategy::Dockerfile => "dockerfile",
        SourceBuildStrategy::Railpack => "railpack",
    };
    Ok(NewJob {
        work_type: WorkType::Build,
        kind: WorkKind::Container,
        os: None,
        architecture: None,
        project: Some(target.project),
        repository: Some(WorkRepository {
            forge: "github".into(),
            owner: name.owner().into(),
            name: name.name().into(),
        }),
        parameters: json!({
            "recipe":"source_image_v1","commit":commit.as_str(),"target":target_name,
            "strategy":strategy,"trigger":"operator","origin":"other"
        }),
        secrets: (target.build_visibility == BuildVisibility::Private)
            .then(|| REPOSITORY_TOKEN.to_owned())
            .into_iter()
            .collect(),
        limits: Limits {
            seconds: 3900,
            idle_seconds: 300,
            memory_bytes: u64::from(memory) * MIB,
            disk_bytes: 21 * 1024 * MIB,
            orphan_seconds: 90,
        },
        machine: None,
    })
}

fn checkout_source(target: &Target) -> Result<Option<CheckoutSource>> {
    if target.build_visibility != BuildVisibility::Private {
        return Ok(None);
    }
    let forge_repository = target.external_id.parse().map_err(internal)?;
    Ok(Some(CheckoutSource {
        account: target.account,
        forge_repository,
    }))
}

fn bad(field: &'static str, message: &str) -> async_graphql::Error {
    invalid(&[FieldError::new(field, message)])
}
