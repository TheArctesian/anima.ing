use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use russh::{
    ChannelWriteHalf, Disconnect,
    client::{Handle, Msg},
};
use tokio::time::timeout;

use super::{control, opening::SignedIn};
use crate::fleet::sessions::{
    end::{CLOSE_WAIT, Cause},
    upgrade::close,
};
use crate::fleet::ssh::Observer;

/// Best effort within [`CLOSE_WAIT`]: the client may be gone (K4 Section 8.6).
pub(super) async fn close_client(client: WebSocket, cause: Cause) {
    let end = Message::Text(Utf8Bytes::from(control::end(cause.ending().reason)));
    close(client, Some(end), cause).await;
}

/// Best effort: dropping the handle ends the connection either way.
pub(super) async fn leave_device(handle: Handle<Observer>, channel: &ChannelWriteHalf<Msg>) {
    let _ = timeout(CLOSE_WAIT, async {
        let _ = channel.close().await;
        handle.disconnect(Disconnect::ByApplication, "", "").await
    })
    .await;
}

pub(super) async fn leave_unstarted(signed_in: SignedIn) {
    let SignedIn { handle, channel } = signed_in;
    let (_, writer) = channel.split();
    leave_device(handle, &writer).await;
}
