//! The limits of the relay: an answer stops with the relay, a connection above the limit and
//! a connection with no request are closed, and a refusal closes its connection.

use std::sync::Arc;
use std::time::Duration;

use http_body_util::BodyExt as _;
use hyper::{Method, StatusCode};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

use super::streams::{WAIT, by_hand, read_head, relay_to};
use super::{call, relay, send};
use crate::upstream::{Program, Trust};

#[tokio::test]
async fn sa6_an_answer_on_its_way_stops_when_the_relay_is_dropped() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let closed = Arc::new(Notify::new());
    let seen_closed = closed.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a connection");
        read_head(&mut stream).await;
        let head = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n5\r\nfirst\r\n";
        stream.write_all(head).await.expect("written");
        // The provider sees the end of the connection of the relay.
        let mut rest = Vec::new();
        let _end = stream.read_to_end(&mut rest).await;
        seen_closed.notify_one();
    });
    let relay = relay_to(port, Trust::Plain).await;
    let base = relay.open("thread-25", Program::ClaudeCode).expect("open");
    let answer = send(Method::POST, &format!("{base}/v1/messages"), &[], "{}").await;
    let mut body = answer.into_body();
    let first = tokio::time::timeout(WAIT, body.frame())
        .await
        .expect("a part");
    let first = first.expect("a part").expect("no fault");
    assert_eq!(first.into_data().expect("bytes"), "first");
    drop(relay);
    let rest = tokio::time::timeout(WAIT, body.collect()).await;
    let rest = rest.expect("the stream ends, it does not hang");
    assert!(
        rest.is_err(),
        "an answer that was cut is a fault, not a whole body"
    );
    let provider_saw = tokio::time::timeout(WAIT, closed.notified()).await;
    provider_saw.expect("the connection to the provider is closed");
}

fn address_of(base: &str) -> String {
    let rest = base.trim_start_matches("http://");
    rest.split('/').next().expect("an address").to_owned()
}

#[tokio::test]
async fn sa6_a_connection_above_the_limit_is_closed_and_the_listener_does_not_wait() {
    let (relay, _provider) = relay().await;
    let base = relay.open("thread-26", Program::ClaudeCode).expect("open");
    let address = address_of(&base);
    let mut idle = Vec::new();
    for _ in 0..128 {
        idle.push(TcpStream::connect(&address).await.expect("a connection"));
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut over = TcpStream::connect(&address)
        .await
        .expect("the listener still accepts");
    let mut byte = [0_u8; 1];
    let read = tokio::time::timeout(WAIT, over.read(&mut byte)).await;
    let read = read.expect("closed at once");
    assert!(
        matches!(read, Ok(0) | Err(_)),
        "the connection above the limit is closed"
    );
    drop(idle);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let answer = call(Method::POST, &format!("{base}/v1/messages"), &[], "{}").await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "room comes back when connections end"
    );
}

#[tokio::test(start_paused = true)]
async fn sa6_a_connection_that_sends_no_request_is_closed_after_the_wait() {
    let (relay, _provider) = relay().await;
    let base = relay.open("thread-27", Program::ClaudeCode).expect("open");
    let mut silent = TcpStream::connect(address_of(&base))
        .await
        .expect("a connection");
    let started = tokio::time::Instant::now();
    let mut byte = [0_u8; 1];
    // The clock of the test moves to the next wait that ends: the wait for a head.
    let read = silent.read(&mut byte).await;
    assert!(
        matches!(read, Ok(0) | Err(_)),
        "the relay closed the connection"
    );
    let waited = started.elapsed();
    assert!(waited >= Duration::from_secs(30), "{waited:?}");
}

#[tokio::test]
async fn sa6_a_refusal_closes_its_connection() {
    let (relay, _provider) = relay().await;
    let base = relay.open("thread-28", Program::ClaudeCode).expect("open");
    let request = "GET /nothing HTTP/1.1\r\nhost: h\r\n\r\n".to_owned();
    let answer = by_hand(&base, request).await;
    assert!(answer.starts_with("HTTP/1.1 404"), "{answer}");
    let closes = answer.to_ascii_lowercase().contains("connection: close");
    assert!(closes, "{answer}");
}
