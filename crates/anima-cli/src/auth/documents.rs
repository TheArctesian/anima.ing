//! Identity and revocation documents; browser approval is in `device`.

use graphql_client::GraphQLQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/auth/logout.graphql",
    response_derives = "Debug"
)]
pub struct Logout;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/auth/me.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct Me;
