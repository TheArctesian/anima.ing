//! CMD-004.

use graphql_client::GraphQLQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/tags/list.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct TagList;
