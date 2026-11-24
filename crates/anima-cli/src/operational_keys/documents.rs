//! The operational-key documents (K-E5 Section 2.2, CMD-004). No answer holds a value; the
//! variables of `SecretAdd` and `SecretRotate` hold one, so they derive no `Debug` (CMD-014).

use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretList;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretShow;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretSet;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretRevoke;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretRemove;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretGrant;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct SecretUngrant;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct SecretAdd;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/operational_keys/documents.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct SecretRotate;
