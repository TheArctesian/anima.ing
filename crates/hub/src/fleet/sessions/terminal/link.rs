use std::sync::Arc;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use russh::{ChannelMsg, ChannelReadHalf, ChannelWriteHalf, client::Msg};
use tokio::time::Instant;

use super::{control, relay::Writing, route::Timing};
use crate::fleet::sessions::{Tunnel, end::End, quiet::Quiet, upgrade};

/// One upgraded terminal (K4 Section 8.4).
pub(super) struct Link<'a> {
    pub(super) client: WebSocket,
    pub(super) reader: ChannelReadHalf,
    pub(super) writer: Arc<ChannelWriteHalf<Msg>>,
    pub(super) tunnel: &'a mut Tunnel,
    pub(super) timing: Timing,
    /// The hub's silence earns the client a `ping`.
    pub(super) hub_quiet: Quiet,
    /// The client's silence ends the session.
    pub(super) client_quiet: Quiet,
    pub(super) writing: Option<Writing>,
    pub(super) write_due: Instant,
}

impl Link<'_> {
    /// Relays standard output and standard error (`ext` 1) unchanged and never keeps them (RA-008).
    pub(super) async fn output(&mut self, message: ChannelMsg) -> Result<(), End> {
        match message {
            ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, ext: 1 } => {
                self.send(Message::Binary(data)).await
            }
            _ => Ok(()),
        }
    }

    pub(super) async fn ping(&mut self) -> Result<(), End> {
        self.send(Message::Text(Utf8Bytes::from_static(control::PING)))
            .await
    }

    pub(super) async fn send(&mut self, message: Message) -> Result<(), End> {
        let wait = self.timing.client_idle;
        upgrade::send(&mut self.client, message, self.tunnel, wait).await?;
        self.hub_quiet.restart(Instant::now());
        Ok(())
    }
}
