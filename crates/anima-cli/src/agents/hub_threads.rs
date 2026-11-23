use graphql_client::GraphQLQuery;

use crate::{
    devices::get::resolve,
    failure::Failure,
    hub::{
        Hub,
        scalars::{DateTime, JSON},
        subscription::Subscription,
    },
    invocation::Invocation,
    output::{
        Cell, Table,
        stream::{self, Reader},
    },
};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/hub_threads.graphql",
    response_derives = "Serialize"
)]
struct HubThreads;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/hub_threads.graphql",
    response_derives = "Serialize"
)]
struct HubThread;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/agents/hub_threads.graphql",
    response_derives = "Serialize"
)]
struct WatchHubThread;

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    #[arg(long)]
    device: Option<String>,
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(i64).range(1..=200))]
    first: i64,
    #[arg(long)]
    after: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ReadArgs {
    id: String,
}

#[derive(Debug, clap::Args)]
pub struct WatchArgs {
    id: String,
    #[arg(long, default_value = "0")]
    after: String,
}

pub(crate) async fn list(
    hub: &Hub,
    (device, program): (Option<&str>, Option<&str>),
    first: i64,
    after: Option<String>,
) -> Result<hub_threads::HubThreadsAgentThreads, Failure> {
    let device = match device {
        Some(name) => Some(resolve(hub, name).await?),
        None => None,
    };
    Ok(hub
        .run::<HubThreads>(hub_threads::Variables {
            device,
            program: program.map(str::to_owned),
            first,
            after,
        })
        .await?
        .agent_threads)
}

impl ListArgs {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let answer = list(&hub, (self.device.as_deref(), None), self.first, self.after).await?;
        invocation.output.data(&answer, |answer| {
            let mut table = Table::new(vec!["ID", "DEVICE", "PROGRAM", "STATE", "TITLE"]);
            for row in &answer.nodes {
                table.row(vec![
                    Cell::new(&row.id),
                    Cell::new(&row.device_id),
                    Cell::new(&row.program),
                    Cell::new(&row.state),
                    Cell::new(&row.title),
                ]);
            }
            table
        })
    }
}

impl ReadArgs {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let snapshot = hub
            .run::<HubThread>(hub_thread::Variables { id: self.id })
            .await?;
        invocation.output.data(&snapshot.agent_thread, |snapshot| {
            let mut table = Table::new(vec!["SEQUENCE", "ENTRY"]);
            if let Some(snapshot) = snapshot {
                for entry in &snapshot.entries {
                    table.row(vec![
                        Cell::new(&entry.sequence),
                        Cell::new(&entry.content.to_string()),
                    ]);
                }
            }
            table
        })
    }
}

impl WatchArgs {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let variables = watch_hub_thread::Variables {
            id: self.id,
            after: self.after,
        };
        let mut updates = Subscription::open::<WatchHubThread>(&hub, variables).await?;
        while let Some(update) = updates.next().await {
            let entry = update?.agent_thread_entries;
            if stream::emit(invocation.output.json(), &entry, |entry| {
                format!("{}  {}", entry.sequence, entry.content)
            })? == Reader::Gone
            {
                break;
            }
        }
        Ok(())
    }
}
