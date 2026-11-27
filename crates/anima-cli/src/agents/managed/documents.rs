use crate::hub::scalars::JSON;
use graphql_client::GraphQLQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedWorkspaces;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedReceipt;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedEnable;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedSubmit;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedPreviews;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedPreviewOpen;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedPreviewClose;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct WorkspaceInventory;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct WorkspaceReceipt;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct WorkspaceSubmit;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedProviderLogin;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/managed/operations.graphql",
    response_derives = "Serialize"
)]
pub struct ManagedProviderReply;
