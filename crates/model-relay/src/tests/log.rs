//! What never reaches a log (Section 7.4).

use std::io::Write;
use std::sync::{Arc, Mutex};

use hyper::{Method, StatusCode};

use super::{call, relay, seen};
use crate::Relay;
use crate::upstream::{Program, Trust, Upstream, Upstreams};

/// The log lines of a test, in memory.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<u8>>>);

impl Write for Lines {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("the lines").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn sa6_no_sign_in_data_no_secret_and_no_body_reach_a_log() {
    let lines = Lines::default();
    let writer = lines.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let (relay, provider) = relay().await;
    let base = relay.open("thread-7", Program::ClaudeCode).expect("open");
    let secret = base.rsplit('/').next().expect("a secret").to_owned();
    let headers = [
        ("authorization", "Bearer MARK-TOKEN"),
        ("x-api-key", "MARK-KEY"),
        ("cookie", "session=MARK-COOKIE"),
        ("origin-less", "MARK-HEADER"),
    ];
    call(
        Method::POST,
        &format!("{base}/v1/messages?q=MARK-QUERY"),
        &headers,
        "MARK-BODY",
    )
    .await;
    call(Method::GET, &format!("{base}/v1/moved"), &headers, "").await;
    call(
        Method::GET,
        &format!("{base}/v1/messages"),
        &[
            ("upgrade", "websocket"),
            ("authorization", "Bearer MARK-TOKEN"),
        ],
        "",
    )
    .await;
    relay.close("thread-7");
    assert_eq!(seen(&provider).len(), 2);
    let log = String::from_utf8_lossy(&lines.0.lock().expect("the lines")).into_owned();
    assert!(
        log.contains("thread-7"),
        "the relay logs each request by its thread: {log}"
    );
    for mark in ["MARK-", secret.as_str(), "Bearer", "/v1/messages"] {
        assert!(!log.contains(mark), "{mark} is in the log: {log}");
    }
}

#[tokio::test]
async fn sa6_a_provider_that_is_not_reached_gives_502_and_no_sign_in_data_in_the_log() {
    let lines = Lines::default();
    let writer = lines.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let dead = Upstream {
        host: "127.0.0.1".to_owned(),
        port: 1,
        trust: Trust::Plain,
        prefix: "",
    };
    let upstreams = Upstreams {
        anthropic: dead.clone(),
        openai: dead.clone(),
        chatgpt: dead,
    };
    let broken = Relay::start_with(upstreams).await.expect("a relay");
    let base = broken.open("thread-8", Program::ClaudeCode).expect("open");
    let headers = [
        ("authorization", "Bearer MARK-TOKEN"),
        ("x-api-key", "MARK-KEY"),
    ];
    let unreachable = call(
        Method::POST,
        &format!("{base}/v1/messages"),
        &headers,
        "MARK-BODY",
    )
    .await;
    assert_eq!(unreachable.status, StatusCode::BAD_GATEWAY);
    assert!(!unreachable.body.contains("MARK-"));
    let log = String::from_utf8_lossy(&lines.0.lock().expect("the lines")).into_owned();
    assert!(log.contains("thread-8"), "{log}");
    assert!(!log.contains("MARK-") && !log.contains("Bearer"), "{log}");
}
