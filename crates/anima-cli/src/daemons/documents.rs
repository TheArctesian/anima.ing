//! `DaemonEnrol` has no `Debug`: its answer holds the one-time enrolment code (CMD-014, LOG-007).

use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/daemons/status.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct DaemonStatus;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/daemons/enrol.graphql",
    response_derives = "Serialize",
    variables_derives = "Debug"
)]
pub struct DaemonEnrol;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/daemons/revoke.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct DaemonRevoke;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/daemons/need.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug"
)]
pub struct DaemonNeedSet;
