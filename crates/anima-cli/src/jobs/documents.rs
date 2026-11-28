use graphql_client::GraphQLQuery;

use crate::hub::scalars::DateTime;

macro_rules! documents {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "schema.graphql",
            query_path = "src/jobs/work.graphql",
            response_derives = "Serialize",
            variables_derives = "Deserialize"
        )]
        pub(crate) struct $name;
    )+};
}

documents!(
    JobList,
    JobDetail,
    JobStop,
    JobDeviceWork,
    JobProjectDevices,
    JobProjectMachine,
    JobDeviceExcluded,
);
