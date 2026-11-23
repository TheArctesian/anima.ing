//! The `sftp` subsystem on a link (RA-032, K4 Section 9). Nothing here logs.

use std::time::Duration;

use russh::{ChannelMsg, ChannelStream, client::Msg};
use tokio::time::timeout;

use super::{Error, SshLink};

const SFTP: &str = "sftp";

impl SshLink {
    /// No PTY, shell, `exec`, environment, agent or forwarding request is sent.
    /// The link must outlive the stream.
    pub(crate) async fn sftp(&self, limit: Duration) -> Result<ChannelStream<Msg>, Error> {
        timeout(limit, async {
            let mut channel =
                self.handle
                    .channel_open_session()
                    .await
                    .map_err(|error| match error {
                        russh::Error::ChannelOpenFailure(_) => Error::CommandRefused,
                        _ => Error::Lost,
                    })?;
            channel
                .request_subsystem(true, SFTP)
                .await
                .map_err(|_| Error::Lost)?;
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Success) => return Ok(channel.into_stream()),
                    Some(ChannelMsg::Failure | ChannelMsg::Close | ChannelMsg::Eof) => {
                        return Err(Error::CommandRefused);
                    }
                    Some(_) => {}
                    None => return Err(Error::Lost),
                }
            }
        })
        .await
        .map_err(|_| Error::TimeLimit)?
    }
}
