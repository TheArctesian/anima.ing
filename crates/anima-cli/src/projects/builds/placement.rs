//! Inspect or change the build-device pin using an operator session.
use graphql_client::GraphQLQuery;

use crate::{failure::Failure, invocation::Invocation, output::Table};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/projects/builds/placement.graphql",
    response_derives = "Debug, Serialize"
)]
struct ProjectBuildPlacementGet;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/projects/builds/set_pin.graphql",
    response_derives = "Debug, Serialize"
)]
struct ProjectBuildPinSet;

pub(super) async fn show(invocation: &Invocation, project: String) -> Result<(), Failure> {
    let hub = invocation.hub()?;
    let result = hub
        .run::<ProjectBuildPlacementGet>(project_build_placement_get::Variables { project })
        .await?;
    invocation
        .output
        .data(&result.project_build_placement, |placement| {
            record(
                placement
                    .pinned
                    .as_ref()
                    .map(|node| label(&node.name, &node.id, node.private_checkout)),
                placement
                    .candidates
                    .iter()
                    .map(|node| label(&node.name, &node.id, node.private_checkout)),
            )
        })
}

pub(super) async fn set(
    invocation: &Invocation,
    project: String,
    device: Option<String>,
) -> Result<(), Failure> {
    let hub = invocation.hub()?;
    let result = hub
        .run::<ProjectBuildPinSet>(project_build_pin_set::Variables { project, device })
        .await?;
    invocation
        .output
        .data(&result.set_project_build_pin, |placement| {
            record(
                placement
                    .pinned
                    .as_ref()
                    .map(|node| label(&node.name, &node.id, node.private_checkout)),
                placement
                    .candidates
                    .iter()
                    .map(|node| label(&node.name, &node.id, node.private_checkout)),
            )
        })
}

fn label(name: &str, id: &str, private: bool) -> String {
    if private {
        format!("{name} ({id}) · private checkout")
    } else {
        format!("{name} ({id})")
    }
}

fn record(pinned: Option<String>, candidates: impl Iterator<Item = String>) -> Table {
    let mut table = Table::record();
    table.pair("Placement", pinned.unwrap_or_else(|| "Automatic".into()));
    for candidate in candidates {
        table.pair("Node", candidate);
    }
    table
}
