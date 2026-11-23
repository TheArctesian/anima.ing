//! The documents of the copies, typed from K1 at build time (CMD-004).

use graphql_client::GraphQLQuery;

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
