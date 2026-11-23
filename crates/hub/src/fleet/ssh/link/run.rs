//! One command on a link (D-3, CHK-012); nothing here logs or keeps the output past the call.

use std::time::Duration;

use russh::{
    Channel, ChannelMsg,
    client::{Handle, Msg},
};
use tokio::time::timeout;

use super::{CommandOutput, Error, OUTPUT_CAP};
use crate::fleet::ssh::Observer;

const CLOSE_WAIT: Duration = Duration::from_secs(1);

/// Closing the channel ends the command on the device and frees the link for the next one.
pub(super) async fn run(
    handle: &Handle<Observer>,
    line: &str,
    limit: Duration,
) -> Result<CommandOutput, Error> {
    let mut opened = None;
    let ran = timeout(limit, async {
        let channel = opened.insert(open(handle).await?);
        exchange(channel, line).await
    })
    .await
    .map_err(|_| Error::TimeLimit);
    if let Some(channel) = opened {
        // A failed close is the next command's error; this command's result stands.
        let _closed = timeout(CLOSE_WAIT, channel.close()).await;
    }
    ran?
}

async fn open(handle: &Handle<Observer>) -> Result<Channel<Msg>, Error> {
    handle
        .channel_open_session()
        .await
        .map_err(|error| match error {
            russh::Error::ChannelOpenFailure(_) => Error::CommandRefused,
            _ => Error::Lost,
        })
}

async fn exchange(channel: &mut Channel<Msg>, line: &str) -> Result<CommandOutput, Error> {
    channel.exec(true, line).await.map_err(|_| Error::Lost)?;
    let mut output = CommandOutput {
        exit_status: None,
        stdout: Vec::new(),
        cut: false,
    };
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Failure => return Err(Error::CommandRefused),
            ChannelMsg::Data { data } => {
                if !kept(&mut output.stdout, &data) {
                    output.cut = true;
                    output.exit_status = None;
                    return Ok(output);
                }
            }
            ChannelMsg::ExitStatus { exit_status } => output.exit_status = Some(exit_status),
            ChannelMsg::Close => return Ok(output),
            _ => {}
        }
    }
    Err(Error::Lost)
}

/// `false` when some of `data` did not fit under [`OUTPUT_CAP`].
pub(super) fn kept(stdout: &mut Vec<u8>, data: &[u8]) -> bool {
    let room = OUTPUT_CAP.saturating_sub(stdout.len());
    let fits = data.len() <= room;
    stdout.extend_from_slice(&data[..data.len().min(room)]);
    fits
}
