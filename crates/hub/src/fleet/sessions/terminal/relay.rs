//! The running terminal (K4 Sections 8.4 to 8.6); each `select!` branch is cancel safe and no lock is held.
//! Backpressure (K4 Section 8.5): one frame and one client write in flight, so a slow client stalls the channel.
//! A write waiting on the device is not the client's silence; one past the request wait ends `GATEWAY_ERROR`.

use std::{pin::Pin, sync::Arc};

use axum::extract::ws::Message;
use futures_util::FutureExt as _;
use russh::{ChannelMsg, client::Handle};
use tokio::time::{Instant, sleep_until, timeout};

use super::{
    control::{self, Control, Size},
    ending::{DEVICE_STOPPED_READING, ended},
    link::Link,
};
use crate::fleet::sessions::{
    EndCause,
    end::{CLOSE_WAIT, Cause, End, unreadable},
};
use crate::fleet::ssh::Observer;

pub(super) type Writing = Pin<Box<dyn Future<Output = Result<(), russh::Error>> + Send>>;

enum Turn {
    Signalled(EndCause),
    Client(Option<Result<Message, axum::Error>>),
    Written(Result<(), russh::Error>),
    Device(Option<ChannelMsg>),
    HubSilent,
    ClientIdle,
    WriteStalled,
}

impl Link<'_> {
    pub(super) async fn relay(&mut self, handle: &mut Handle<Observer>) -> End {
        self.client_quiet.restart(Instant::now());
        loop {
            let turn = self.next_turn().await;
            if let Some(end) = self.on_turn(turn, handle).await {
                return end;
            }
        }
    }

    /// Cancel safe: the write in flight stays in `writing` when another branch wins.
    async fn next_turn(&mut self) -> Turn {
        let writing = self.writing.is_some();
        tokio::select! {
            signal = self.tunnel.ended() => Turn::Signalled(signal),
            message = self.client.recv(), if !writing => Turn::Client(message),
            written = written(&mut self.writing), if writing => Turn::Written(written),
            message = self.reader.wait() => Turn::Device(message),
            () = sleep_until(self.hub_quiet.due()) => Turn::HubSilent,
            () = sleep_until(self.client_quiet.due()), if !writing => Turn::ClientIdle,
            () = sleep_until(self.write_due), if writing => Turn::WriteStalled,
        }
    }

    /// A WebSocket ping or pong and the end of a held-back write also restart the client's idle wait.
    async fn on_turn(&mut self, turn: Turn, handle: &mut Handle<Observer>) -> Option<End> {
        match turn {
            Turn::Signalled(signal) => Some(End::from(signal)),
            Turn::Client(message) => self.on_client(message),
            Turn::Written(written) => {
                self.writing = None;
                self.client_quiet.restart(Instant::now());
                written.err().map(|_| ended(Cause::GatewayError))
            }
            Turn::Device(Some(ChannelMsg::Close)) => Some(ended(Cause::RemoteClosed)),
            Turn::Device(Some(message)) => self.output(message).await.err(),
            Turn::Device(None) => Some(gone(handle).await),
            Turn::HubSilent => self.ping().await.err(),
            Turn::ClientIdle => self.client_idle(),
            Turn::WriteStalled => Some(End::explained(Cause::GatewayError, DEVICE_STOPPED_READING)),
        }
    }

    fn on_client(&mut self, message: Option<Result<Message, axum::Error>>) -> Option<End> {
        self.client_quiet.restart(Instant::now());
        let text = match message {
            Some(Ok(Message::Binary(bytes))) => {
                let writer = Arc::clone(&self.writer);
                return self.write(Box::pin(async move { writer.data_bytes(bytes).await }));
            }
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => return None,
            Some(Ok(Message::Close(_))) | None => return Some(ended(Cause::OperatorClosed)),
            Some(Err(error)) => return Some(ended(unreadable(error))),
        };
        match control::parse(&text) {
            Ok(Control::Resize(size)) => {
                let writer = Arc::clone(&self.writer);
                let Size { cols, rows } = size;
                let change = async move {
                    let (cols, rows) = (u32::from(cols), u32::from(rows));
                    writer.window_change(cols, rows, 0, 0).await
                };
                self.write(Box::pin(change))
            }
            Ok(Control::Ping) => None,
            Err(_) => Some(ended(Cause::InvalidInstruction)),
        }
    }

    fn write(&mut self, write: Writing) -> Option<End> {
        self.writing = Some(write);
        self.write_due = Instant::now() + self.timing.request;
        None
    }

    /// A message that arrived while the hub was sending is read first, so time held back is not silence.
    fn client_idle(&mut self) -> Option<End> {
        match self.client.recv().now_or_never() {
            Some(message) => self.on_client(message),
            None => Some(ended(Cause::Timeout)),
        }
    }
}

async fn written(writing: &mut Option<Writing>) -> Result<(), russh::Error> {
    match writing {
        Some(write) => write.as_mut().await,
        None => std::future::pending().await,
    }
}

/// An SSH disconnect is the remote closing; anything else is a failed connection (K4 Section 8.6).
async fn gone(handle: &mut Handle<Observer>) -> End {
    match timeout(CLOSE_WAIT, handle).await {
        Ok(Ok(())) => ended(Cause::RemoteClosed),
        Ok(Err(_)) | Err(_) => ended(Cause::GatewayError),
    }
}
