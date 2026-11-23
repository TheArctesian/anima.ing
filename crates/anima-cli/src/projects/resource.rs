//! PRO-004.

use super::documents::{ProjectGet, ResourceConfirm, project_get, resource_confirm};
use super::render;
use crate::failure::{Failure, Hint};
use crate::hub::Hub;
use crate::invocation::Invocation;
use crate::output::text::enum_name;

#[derive(Debug, clap::Subcommand)]
pub enum ResourceCommand {
    /// Confirm the resources of one kind that the hub found in a project.
    Confirm(ConfirmArgs),
}

#[derive(Debug, clap::Args)]
pub struct ConfirmArgs {
    /// The project's slug.
    slug: String,
    /// The kind of resource.
    #[arg(value_enum)]
    kind: KindArg,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum KindArg {
    Cloudflare,
    Vercel,
    Server,
    DesktopApplication,
}

impl KindArg {
    const fn k1(self) -> &'static str {
        match self {
            Self::Cloudflare => "CLOUDFLARE",
            Self::Vercel => "VERCEL",
            Self::Server => "SERVER",
            Self::DesktopApplication => "DESKTOP_APPLICATION",
        }
    }
}

impl ResourceCommand {
    /// # Errors
    /// [`Failure`] for an unknown project, no resource of the kind, or a hub refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Confirm(args) => args.run(invocation).await,
        }
    }
}

impl ConfirmArgs {
    /// With `--json`, prints the K1 `ProjectResource` list.
    ///
    /// # Errors
    /// [`Failure::NotFound`] when the project has no resource of the kind; the hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let confirmed = confirm(&hub, &self.slug, self.kind.k1()).await?;
        invocation.output.data(&confirmed, render::resources)
    }
}

/// Confirms the waiting resources of `kind`, or all of that kind when none waits, so the hub says `CONFLICT`.
///
/// # Errors
/// [`Failure::NotFound`] for an unknown project or no resource of that kind; the hub's refusal.
pub async fn confirm(
    hub: &Hub,
    slug: &str,
    kind: &str,
) -> Result<Vec<resource_confirm::ResourceConfirmConfirmProjectResource>, Failure> {
    let answer = hub
        .run::<ProjectGet>(project_get::Variables {
            slug: slug.to_owned(),
        })
        .await?;
    let project = answer.project.ok_or_else(|| Failure::NotFound {
        message: format!("There is no project with the slug {slug}."),
        hint: Some(Hint::run("anima project list")),
    })?;
    let of_kind: Vec<_> = project
        .resources
        .iter()
        .filter(|resource| enum_name(&resource.kind) == kind)
        .collect();
    if of_kind.is_empty() {
        return Err(Failure::NotFound {
            message: format!("The project {slug} has no {kind} resource."),
            hint: Some(Hint::run(format!("anima project get {slug}"))),
        });
    }
    let waiting: Vec<_> = of_kind
        .iter()
        .filter(|resource| enum_name(&resource.state) != "CONFIRMED")
        .collect();
    let targets = if waiting.is_empty() {
        of_kind.iter().map(|resource| &resource.id).collect()
    } else {
        waiting
            .iter()
            .map(|resource| &resource.id)
            .collect::<Vec<_>>()
    };
    let mut confirmed = Vec::new();
    for id in targets {
        let variables = resource_confirm::Variables { id: id.clone() };
        confirmed.push(
            hub.run::<ResourceConfirm>(variables)
                .await?
                .confirm_project_resource,
        );
    }
    Ok(confirmed)
}
