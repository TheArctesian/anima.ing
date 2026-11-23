use super::channel::resolve;
use super::deliveries::result_cell;
use super::documents::{ChannelTest, channel_test};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::Table;
use crate::output::text::{clean, enum_name};

#[derive(Debug, clap::Args)]
pub struct TestArgs {
    /// The channel's ID or its exact name.
    channel: String,
}

impl TestArgs {
    /// # Errors
    /// [`Failure::NotFound`] for an unknown channel; [`Failure::NotDelivered`] when `ok` is false.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let id = resolve(&hub, &self.channel).await?;
        let answer = hub
            .run::<ChannelTest>(channel_test::Variables { id })
            .await?;
        let delivery = answer.send_test_notification;
        invocation.output.data(&delivery, table)?;
        if delivery.ok {
            return Ok(());
        }
        Err(Failure::NotDelivered {
            message: not_delivered(&delivery),
        })
    }
}

fn not_delivered(delivery: &channel_test::ChannelTestSendTestNotification) -> String {
    let reason = delivery.reason.as_ref().map_or_default(enum_name);
    let code = delivery
        .code
        .map_or_default(|code| format!(", code {code}"));
    format!(
        "The test message to {} was not delivered ({reason}{code}).",
        clean(&delivery.channel_name)
    )
}

fn table(delivery: &channel_test::ChannelTestSendTestNotification) -> Table {
    let mut table = Table::record();
    table.pair("Channel", delivery.channel_name.as_str());
    table.pair("At", delivery.at.as_str());
    table.pair(
        "Result",
        result_cell(delivery.ok, delivery.reason.as_ref(), delivery.code),
    );
    table
}
