//! The operator's client as the app opens it (K4 Section 8).

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        Error, Message,
        client::IntoClientRequest as _,
        http::StatusCode,
        protocol::{
            CloseFrame,
            frame::{
                Frame,
                coding::{CloseCode, Data, OpCode},
            },
        },
    },
};

use super::WAIT;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Received {
    Text(String),
    Binary(Vec<u8>),
    Closed(Option<u16>),
}

/// `received` holds every payload byte so far, pings and close reasons included.
pub(super) struct Socket {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    pub(super) received: Vec<u8>,
}

pub(super) async fn open_with(url: &str, subprotocol: bool) -> Result<Socket, StatusCode> {
    let mut request = url.into_client_request().unwrap();
    if subprotocol {
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", "anima-terminal".parse().unwrap());
    }
    match timeout(WAIT, connect_async(request)).await.unwrap() {
        Ok((stream, response)) => {
            assert_eq!(
                response.headers()["Sec-WebSocket-Protocol"],
                "anima-terminal",
                "the hub echoes the subprotocol"
            );
            Ok(Socket {
                stream,
                received: Vec::new(),
            })
        }
        Err(Error::Http(response)) => {
            assert!(
                response.body().as_ref().is_none_or(Vec::is_empty),
                "a refusal has no body detail"
            );
            Err(response.status())
        }
        Err(other) => panic!("the terminal request failed: {other}"),
    }
}

pub(super) async fn open(url: &str) -> Socket {
    open_with(url, true).await.expect("the terminal opens")
}

impl Socket {
    pub(super) async fn next(&mut self) -> Received {
        loop {
            let message = timeout(WAIT, self.stream.next()).await.expect("a message");
            if let Some(Ok(message)) = &message {
                self.received
                    .extend_from_slice(&message.clone().into_data());
                if let Message::Close(Some(frame)) = message {
                    self.received.extend_from_slice(frame.reason.as_bytes());
                }
            }
            match message {
                Some(Ok(Message::Text(text))) => return Received::Text(text.to_string()),
                Some(Ok(Message::Binary(bytes))) => return Received::Binary(bytes.to_vec()),
                Some(Ok(Message::Close(frame))) => {
                    // A browser sends the queued close echo before reporting the close.
                    let _ = timeout(WAIT, self.stream.flush()).await;
                    return Received::Closed(frame.map(|frame| u16::from(frame.code)));
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Err(_)) | None => return Received::Closed(None),
            }
        }
    }

    pub(super) async fn bytes_until(&mut self, expected: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        while !bytes
            .windows(expected.len())
            .any(|window| window == expected)
        {
            match self.next().await {
                Received::Binary(more) => bytes.extend(more),
                Received::Text(text) if text == r#"{"type":"ping"}"# => {}
                other => panic!("{other:?} while terminal bytes were due"),
            }
        }
        bytes
    }

    pub(super) async fn ended(&mut self) -> (String, Option<u16>) {
        loop {
            match self.next().await {
                Received::Text(text) if text == r#"{"type":"ping"}"# => {}
                Received::Text(text) => {
                    let end: serde_json::Value = serde_json::from_str(&text).unwrap();
                    assert_eq!(end["type"], "end", "{text}");
                    assert_eq!(end.as_object().unwrap().len(), 2, "{text}");
                    let reason = end["reason"].as_str().unwrap().to_owned();
                    return (reason, self.close_code().await);
                }
                Received::Binary(_) => {}
                Received::Closed(code) => panic!("closed with {code:?} before `end`"),
            }
        }
    }

    pub(super) async fn close_code(&mut self) -> Option<u16> {
        loop {
            if let Received::Closed(code) = self.next().await {
                return code;
            }
        }
    }

    /// The hub reads a WebSocket ping after everything sent before it, so its pong marks catch-up.
    pub(super) async fn caught_up(&mut self) {
        let ping = Message::Ping(b"caught up".to_vec().into());
        self.stream.send(ping).await.unwrap();
        loop {
            let message = timeout(WAIT, self.stream.next()).await.expect("a pong");
            if let Some(Ok(message)) = &message {
                self.received
                    .extend_from_slice(&message.clone().into_data());
            }
            match message {
                Some(Ok(Message::Pong(_))) => return,
                Some(Ok(Message::Text(text))) if text == r#"{"type":"ping"}"# => {}
                other => panic!("{other:?} before the pong"),
            }
        }
    }

    pub(super) async fn send(&mut self, text: &str) {
        self.stream.send(Message::text(text)).await.unwrap();
    }

    pub(super) async fn resize(&mut self, cols: u16, rows: u16) {
        let message = format!(r#"{{"type":"resize","cols":{cols},"rows":{rows}}}"#);
        self.send(&message).await;
    }

    pub(super) async fn send_binary(&mut self, bytes: &[u8]) {
        self.stream
            .send(Message::binary(bytes.to_vec()))
            .await
            .unwrap();
    }

    pub(super) async fn send_unchecked_text(&mut self, bytes: &[u8]) {
        let frame = Frame::message(bytes.to_vec(), OpCode::Data(Data::Text), true);
        self.stream.send(Message::Frame(frame)).await.unwrap();
    }

    pub(super) async fn send_fragmented(&mut self, bytes: &[u8], parts: usize) {
        let size = bytes.len().div_ceil(parts);
        let chunks: Vec<&[u8]> = bytes.chunks(size).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let opcode = if index == 0 {
                OpCode::Data(Data::Binary)
            } else {
                OpCode::Data(Data::Continue)
            };
            let frame = Frame::message(chunk.to_vec(), opcode, index + 1 == chunks.len());
            self.stream.send(Message::Frame(frame)).await.unwrap();
        }
    }

    /// The hub's close echoes the code (RFC 6455 Section 5.5.1).
    pub(super) async fn close(&mut self) -> Option<u16> {
        let normal = CloseFrame {
            code: CloseCode::Normal,
            reason: "".into(),
        };
        self.stream.close(Some(normal)).await.unwrap();
        self.close_code().await
    }
}

/// A raw connection kept open, so the test can leave while the hub is still opening.
pub(super) async fn request(served: std::net::SocketAddr, token: &str) -> TcpStream {
    let mut raw = TcpStream::connect(served).await.unwrap();
    let request = format!(
        "GET /terminal?token={token} HTTP/1.1\r\nHost: {served}\r\nConnection: Upgrade\r\n\
         Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: anima-terminal\r\n\r\n"
    );
    raw.write_all(request.as_bytes()).await.unwrap();
    raw
}

pub(super) async fn status_line(raw: &mut TcpStream) -> String {
    let mut response = [0; 64];
    let read = timeout(WAIT, raw.read(&mut response))
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&response[..read]).into_owned();
    text.lines().next().unwrap_or_default().to_owned()
}
