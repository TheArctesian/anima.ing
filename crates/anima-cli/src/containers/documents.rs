//! The container documents, typed from K1 at build time (CMD-004).

use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/list.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerList;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/log.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerLogRead;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/inspect.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerInspect;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/storage.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerStorageRead;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/networks.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerNetworkList;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/updates.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerUpdateList;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/check_file.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ComposeFileCheckRun;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/from_run.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ComposeFromDockerRunRead;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/action.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct QueueContainerAction;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/action_status.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ContainerActionStatus;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/apply.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct QueueApplicationApply;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/apply_status.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct ApplicationApplyStatus;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/containers/apply_targets.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct ApplicationApplyTargets;
