//! The API key documents (CMD-004). `KeyCreate` derives neither `Debug` nor `Serialize`: its
//! answer holds the new secret (CMD-014).

use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/keys/list.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct KeyList;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/keys/create.graphql",
    skip_serializing_none
)]
pub struct KeyCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/keys/revoke.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct KeyRevoke;
