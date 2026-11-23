//! The documents of the copies, typed from K1 at build time (CMD-004).

use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_copies/copies.graphql",
    response_derives = "Debug, Serialize, Clone",
    variables_derives = "Debug"
)]
pub struct ModelCopies;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_copies/pull.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ModelPull;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_copies/remove.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct ModelCopyRemove;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_copies/found.graphql",
    response_derives = "Debug, Serialize, Clone",
    variables_derives = "Debug"
)]
pub struct FoundModels;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_copies/find.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct FindModels;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/model_copies/use.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct UseFoundModel;
