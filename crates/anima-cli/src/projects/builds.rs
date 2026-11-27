//! Source image builds for signed-in project operators.
use graphql_client::GraphQLQuery;

mod download;
mod image;
mod placement;

use crate::{failure::Failure, invocation::Invocation, output::Table};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/projects/queue_build.graphql",
    response_derives = "Debug, Serialize",
    skip_serializing_none
)]
struct SourceBuildQueue;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/projects/build_status.graphql",
    response_derives = "Debug, Serialize"
)]
struct SourceBuildStatus;

#[derive(Debug, clap::Subcommand)]
pub enum BuildCommand {
    /// Queue a project's exact commit.
    Queue(QueueArgs),
    /// Read a previously queued source build.
    Status { id: String },
    /// Show the project's build pin and candidate node IDs.
    Placement { project: String },
    /// Pin the project's builds to an enrolled node.
    Pin { project: String, device: String },
    /// Clear the project's build node.
    Unpin { project: String },
    /// Save the verified OCI archive of one completed attempt.
    Download(download::DownloadArgs),
    /// Verify and show the exact runnable image of a completed build attempt.
    InspectImage(image::ImageArgs),
    /// Copy a confirmed image by digest to the hub's configured private registry.
    PublishImage(image::PublishArgs),
}

#[derive(Debug, clap::Args)]
pub struct QueueArgs {
    /// A repository ID from `anima project get --json`.
    repository: String,
    /// The full 40-digit Git commit.
    commit: String,
    #[arg(long, value_enum)]
    target: Target,
    #[arg(long, value_enum, default_value = "auto")]
    strategy: Strategy,
    /// Build memory, from 512 to 2048 MiB. The hub defaults to 2048.
    #[arg(long, value_parser = clap::value_parser!(u32).range(512..=2048))]
    memory_mib: Option<u32>,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Target {
    #[value(name = "linux-x86_64")]
    X86_64,
    #[value(name = "linux-aarch64")]
    Aarch64,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Strategy {
    Auto,
    Dockerfile,
    Railpack,
}

impl BuildCommand {
    /// Run a source build command against the hub.
    ///
    /// # Errors
    /// Returns a failure if the request, authentication or output fails.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Download(args) => args.run(invocation).await,
            Self::InspectImage(args) => args.run(invocation).await,
            Self::PublishImage(args) => args.run(invocation).await,
            Self::Queue(args) => args.run(invocation).await,
            Self::Placement { project } => placement::show(invocation, project).await,
            Self::Pin { project, device } => {
                placement::set(invocation, project, Some(device)).await
            }
            Self::Unpin { project } => placement::set(invocation, project, None).await,
            Self::Status { id } => {
                let hub = invocation.hub()?;
                let result = hub
                    .run::<SourceBuildStatus>(source_build_status::Variables { id })
                    .await?;
                let status = result.source_build.ok_or_else(|| Failure::NotFound {
                    message: "No source build has that ID.".into(),
                    hint: None,
                })?;
                invocation.output.data(&status, |status| {
                    record(
                        &status.id,
                        &status.state,
                        status.attempt,
                        status.reason.as_deref(),
                    )
                })
            }
        }
    }
}

impl QueueArgs {
    async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        use source_build_queue::{
            SourceBuildInput, SourceBuildStrategy, SourceBuildTarget, Variables,
        };
        let target = match self.target {
            Target::X86_64 => SourceBuildTarget::LINUX_X86_64,
            Target::Aarch64 => SourceBuildTarget::LINUX_AARCH64,
        };
        let strategy = match self.strategy {
            Strategy::Auto => SourceBuildStrategy::AUTO,
            Strategy::Dockerfile => SourceBuildStrategy::DOCKERFILE,
            Strategy::Railpack => SourceBuildStrategy::RAILPACK,
        };
        let hub = invocation.hub()?;
        let result = hub
            .run::<SourceBuildQueue>(Variables {
                input: SourceBuildInput {
                    request: None,
                    repository: self.repository,
                    commit: self.commit,
                    target,
                    strategy,
                    memory_mib: self.memory_mib.map(i64::from),
                },
            })
            .await?;
        let build = result.queue_source_build;
        invocation.output.data(&build, |build| {
            record(
                &build.id,
                &build.state,
                build.attempt,
                build.reason.as_deref(),
            )
        })
    }
}

fn record(id: &str, state: &str, attempt: i64, reason: Option<&str>) -> Table {
    let mut table = Table::record();
    table.pair("ID", id);
    table.pair("State", state);
    table.pair("Attempt", attempt.to_string());
    if let Some(reason) = reason {
        table.pair("Reason", reason);
    }
    table
}
