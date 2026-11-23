//! A send races the end signal, so a client that stops reading cannot hide an end (D-9).

use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket};
use futures_util::SinkExt;
use tokio::time::timeout;

use crate::fleet::sessions::{
    Tunnel,
    end::{CLIENT_STOPPED_READING, CLOSE_WAIT, Cause, End},
};

/// A send that stalls past `wait` is a timeout (K4 Sections 4 and 8.5).
pub(in crate::fleet::sessions) async fn send(
    client: &mut WebSocket,
    message: Message,
    tunnel: &mut Tunnel,
    wait: Duration,
) -> Result<(), End> {
    tokio::select! {
        signal = tunnel.ended() => Err(End::from(signal)),
        sent = timeout(wait, client.send(message)) => match sent {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(Cause::OperatorClosed.into()),
            Err(_) => Err(End::worded(Cause::Timeout, CLIENT_STOPPED_READING)),
        },
    }
}

/// When the client closed first, the library's echo of its close goes out (RFC 6455 Section 5.5.1).
pub(in crate::fleet::sessions) async fn close(
    mut client: WebSocket,
    last: Option<Message>,
    cause: Cause,
) {
    let frame = CloseFrame {
        code: cause.ending().close_code,
        reason: Utf8Bytes::default(),
    };
    let _ = timeout(CLOSE_WAIT, async {
        if let Some(last) = last {
            let _ = client.send(last).await;
        }
        let _ = client.send(Message::Close(Some(frame))).await;
        let _ = SinkExt::close(&mut client).await;
        // A finished closing handshake keeps Windows from resetting the socket and losing the close.
        // After a protocol violation the hub waits instead: an oversized frame is never read into memory.
        if cause == Cause::InvalidInstruction {
            tokio::time::sleep(CLOSE_WAIT).await;
        } else {
            while let Some(Ok(message)) = client.recv().await {
                if matches!(message, Message::Close(_)) {
                    break;
                }
            }
        }
    })
    .await;
}
