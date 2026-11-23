//! `GithubConnectOpen` holds a one-time link, so it derives no `Debug` or `Serialize` (MCP-004, LOG-007).

use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/sources/connect/state.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct GithubConnectState;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/sources/connect/start.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug",
    skip_serializing_none
)]
pub struct GithubConnectStart;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/sources/connect/open.graphql"
)]
pub struct GithubConnectOpen;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/sources/connect/cancel.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct GithubConnectCancel;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/sources/connect/changed.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct GithubConnectChanged;

/// The variables hold the API key, so they derive no `Debug`, and the caller wipes them after the
/// request (CMD-014, LOG-007).
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/sources/connect/dawarich.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct SourceConnectDawarich;
