use graphql_client::GraphQLQuery;

use crate::{failure::Failure, hub::scalars::JSON, invocation::Invocation, output::Table};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/devices/os_update.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct DeviceOsUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/devices/os_update.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct RequestDeviceOsUpdate;

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum Action {
    #[default]
    Status,
    Check,
    Download,
    Activate,
    Rollback,
}

#[derive(Debug, clap::Args)]
pub struct OsUpdateArgs {
    /// The device's ID or exact name.
    device: String,
    #[arg(value_enum, default_value = "status")]
    action: Action,
    /// Confirm that activation or rollback restarts the device.
    #[arg(long)]
    confirm_restart: bool,
    /// The selected action's review revision from the device's update status.
    #[arg(long)]
    review_revision: Option<String>,
}

impl OsUpdateArgs {
    /// # Errors
    /// The hub refuses missing trust, unavailable SSH or an unconfirmed restart.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        if matches!(self.action, Action::Activate | Action::Rollback)
            && (!self.confirm_restart || self.review_revision.is_none())
        {
            return Err(Failure::Usage {
                message: "Review the device's update status, then provide --confirm-restart and --review-revision.".to_owned(),
                hint: None,
            });
        }
        let hub = invocation.hub()?;
        let device_id = super::get::resolve(&hub, &self.device).await?;
        let action = match self.action {
            Action::Status => None,
            Action::Check => Some(request_device_os_update::OsUpdateAction::CHECK),
            Action::Download => Some(request_device_os_update::OsUpdateAction::DOWNLOAD),
            Action::Activate => Some(request_device_os_update::OsUpdateAction::ACTIVATE),
            Action::Rollback => Some(request_device_os_update::OsUpdateAction::ROLLBACK),
        };
        let value = if let Some(action) = action {
            let result = hub
                .run::<RequestDeviceOsUpdate>(request_device_os_update::Variables {
                    device_id,
                    action,
                    confirmed: self.confirm_restart,
                    review_revision: self.review_revision,
                })
                .await?;
            serde_json::json!({"requested":result.request_device_os_update})
        } else {
            hub.run::<DeviceOsUpdate>(device_os_update::Variables { device_id })
                .await?
                .device_os_update
        };
        invocation.output.data(&value, |value| {
            let mut table = Table::record();
            table.pair("OS update", value.to_string());
            table
        })
    }
}
