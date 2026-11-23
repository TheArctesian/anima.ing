//! `anima device watch` (K1 `deviceStatusChanged`).

use graphql_client::GraphQLQuery;

use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::hub::scalars::DateTime;
use crate::hub::subscription::Subscription;
use crate::invocation::Invocation;
use crate::output::stream::{self, Reader};
use crate::output::text::enum_name;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/devices/watch/status.graphql",
    response_derives = "Debug, Serialize",
    variables_derives = "Debug",
    skip_serializing_none
)]
pub struct DeviceStatusChanged;

#[derive(Debug, clap::Args)]
pub struct WatchArgs {
    /// Only this device, by ID or exact name.
    #[arg(long)]
    device: Option<String>,
}

impl WatchArgs {
    /// # Errors
    /// [`Failure`] for an unknown device, a refused token or a lost connection.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let device_id = match &self.device {
            Some(device) => Some(resolve(&hub, device).await?),
            None => None,
        };
        let variables = device_status_changed::Variables { device_id };
        let mut changes = Subscription::open::<DeviceStatusChanged>(&hub, variables).await?;
        while let Some(answer) = changes.next().await {
            let change = answer?.device_status_changed;
            if stream::emit(invocation.output.json(), &change, line)? == Reader::Gone {
                break;
            }
        }
        Ok(())
    }
}

fn line(change: &device_status_changed::DeviceStatusChangedDeviceStatusChanged) -> String {
    format!(
        "{}  {}  {}",
        change.at,
        change.device_id,
        enum_name(&change.status)
    )
}
