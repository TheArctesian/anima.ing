//! The streams: an answer passes part by part through TLS, an upload in parts passes whole, a
//! provider that stops in the middle is seen as a fault, and a certificate that the roots do
//! not sign is refused. Each provider here speaks by hand on a socket.

use std::sync::Arc;
use std::time::Duration;

use http_body_util::BodyExt as _;
use hyper::{Method, StatusCode};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio_rustls::TlsAcceptor;

use super::{call, relay, seen, send};
use crate::Relay;
use crate::upstream::{Program, Trust, Upstream, Upstreams};

const WAIT: Duration = Duration::from_secs(10);

/// A listener with a certificate for `localhost` that only this test knows.
async fn tls_listener() -> (TcpListener, TlsAcceptor, Vec<u8>) {
    let key = rcgen::KeyPair::generate().expect("a key");
    let certificate = rcgen::CertificateParams::new(vec!["localhost".to_owned()])
        .expect("a name")
        .self_signed(&key)
        .expect("a certificate");
    let der = certificate.der().to_vec();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("TLS")
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(der.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        )
        .expect("a certificate that fits its key");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    (listener, TlsAcceptor::from(Arc::new(config)), der)
}

/// A relay whose provider for Claude Code is `localhost` at `port`, known as `trust` says.
async fn relay_to(port: u16, trust: Trust) -> Relay {
    let host = Upstream {
        host: "localhost".to_owned(),
        port,
        trust,
        prefix: "",
    };
    let upstreams = Upstreams {
        anthropic: host.clone(),
        openai: host.clone(),
        chatgpt: host,
    };
    Relay::start_with(upstreams).await.expect("a relay")
}

/// Reads the head of a request from `stream`.
async fn read_head(stream: &mut (impl AsyncRead + Unpin)) {
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await.expect("a byte") == 0 {
            return;
        }
        head.push(byte[0]);
    }
}

/// Answers one request in two parts, the second only after `gate` opens.
async fn two_parts(mut stream: impl AsyncRead + AsyncWrite + Unpin, gate: Arc<Notify>) {
    read_head(&mut stream).await;
    let head = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n5\r\nfirst\r\n";
    stream.write_all(head).await.expect("written");
    stream.flush().await.expect("sent");
    gate.notified().await;
    stream
        .write_all(b"6\r\nsecond\r\n0\r\n\r\n")
        .await
        .expect("written");
    stream.flush().await.expect("sent");
}

#[tokio::test]
async fn sa6_an_answer_passes_part_by_part_through_tls_and_is_not_held() {
    let (listener, acceptor, certificate) = tls_listener().await;
    let port = listener.local_addr().expect("an address").port();
    let gate = Arc::new(Notify::new());
    let open = gate.clone();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("a connection");
        let secure = acceptor.accept(stream).await.expect("TLS");
        two_parts(secure, open).await;
    });
    let relay = relay_to(port, Trust::Root(certificate)).await;
    let base = relay.open("thread-20", Program::ClaudeCode).expect("open");
    let answer = send(Method::POST, &format!("{base}/v1/messages"), &[], "{}").await;
    assert_eq!(answer.status(), StatusCode::OK);
    let mut body = answer.into_body();
    let first = tokio::time::timeout(WAIT, body.frame())
        .await
        .expect("the first part comes while the provider still holds the second")
        .expect("a part")
        .expect("no fault");
    assert_eq!(first.into_data().expect("bytes"), "first");
    gate.notify_one();
    let rest = tokio::time::timeout(WAIT, body.collect())
        .await
        .expect("the end")
        .expect("no fault");
    assert_eq!(rest.to_bytes(), "second");
}

#[tokio::test]
async fn sa6_a_certificate_that_the_public_roots_do_not_sign_is_refused() {
    let (listener, acceptor, _certificate) = tls_listener().await;
    let port = listener.local_addr().expect("an address").port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            // The handshake fails at the relay; nothing is read here.
            let _refused = acceptor.accept(stream).await;
        }
    });
    let relay = relay_to(port, Trust::WebRoots).await;
    let base = relay.open("thread-21", Program::ClaudeCode).expect("open");
    let headers = [("authorization", "Bearer MARK-TOKEN")];
    let answer = call(
        Method::POST,
        &format!("{base}/v1/messages"),
        &headers,
        "MARK-BODY",
    )
    .await;
    assert_eq!(answer.status, StatusCode::BAD_GATEWAY);
    assert!(!answer.body.contains("MARK-"));
}

#[tokio::test]
async fn sa6_a_provider_that_stops_in_the_middle_of_a_body_is_a_fault_for_the_program() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("an address").port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a connection");
        read_head(&mut stream).await;
        let half = b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\npartial";
        stream.write_all(half).await.expect("written");
        stream.shutdown().await.expect("closed");
    });
    let relay = relay_to(port, Trust::Plain).await;
    let base = relay.open("thread-22", Program::ClaudeCode).expect("open");
    let answer = send(Method::POST, &format!("{base}/v1/messages"), &[], "{}").await;
    assert_eq!(
        answer.status(),
        StatusCode::OK,
        "the head had passed before the fault"
    );
    let body = tokio::time::timeout(WAIT, answer.into_body().collect())
        .await
        .expect("an end");
    assert!(
        body.is_err(),
        "a body that is cut is not given as a whole body"
    );
}

/// Writes `request` by hand to the relay at `base` and reads the answer to its end.
async fn by_hand(base: &str, request: String) -> String {
    let mut answer = Vec::new();
    let mut stream = written(base, &request).await;
    tokio::time::timeout(WAIT, stream.read_to_end(&mut answer))
        .await
        .expect("an end")
        .expect("read");
    String::from_utf8_lossy(&answer).into_owned()
}

/// As [`by_hand`], but only the status line start (`HTTP/1.1 404`): the connection stays open.
async fn status_by_hand(base: &str, request: &str) -> String {
    let mut status = [0_u8; 12];
    let mut stream = written(base, request).await;
    tokio::time::timeout(WAIT, stream.read_exact(&mut status))
        .await
        .expect("a status")
        .expect("read");
    String::from_utf8_lossy(&status).into_owned()
}

async fn written(base: &str, request: &str) -> TcpStream {
    let address = base
        .trim_start_matches("http://")
        .split('/')
        .next()
        .expect("an address");
    let mut stream = TcpStream::connect(address).await.expect("a connection");
    stream.write_all(request.as_bytes()).await.expect("written");
    stream
}

fn path_of(base: &str) -> &str {
    let rest = base.trim_start_matches("http://");
    &rest[rest.find('/').expect("a path")..]
}

#[tokio::test]
async fn sa6_an_upload_in_parts_passes_whole() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-23", Program::ClaudeCode).expect("open");
    let request = format!(
        "POST {}/v1/messages HTTP/1.1\r\nhost: h\r\nconnection: close\r\ntransfer-encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        path_of(&base)
    );
    let answer = by_hand(&base, request).await;
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.contains("answer to hello world"), "{answer}");
    let got = seen(&provider);
    assert_eq!(got[0].body, "hello world");
    assert!(
        !got[0].headers.contains_key("content-length") || got[0].headers["content-length"] == "11"
    );
}

#[tokio::test]
async fn sa6_a_request_line_that_names_a_host_does_not_change_where_the_request_goes() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-24", Program::ClaudeCode).expect("open");
    let path = path_of(&base);
    let named = format!(
        "GET http://elsewhere.invalid{path}/v1/models HTTP/1.1\r\nhost: elsewhere.invalid\r\nconnection: close\r\n\r\n"
    );
    assert!(by_hand(&base, named).await.starts_with("HTTP/1.1 200"));
    let tunnel = "CONNECT elsewhere.invalid:443 HTTP/1.1\r\nhost: elsewhere.invalid:443\r\n\r\n";
    assert_eq!(status_by_hand(&base, tunnel).await, "HTTP/1.1 404");
    let got = seen(&provider);
    assert_eq!(
        got.len(),
        1,
        "only the request with the secret went on, to the one provider"
    );
    assert_eq!(got[0].path, "/v1/models");
    assert_ne!(got[0].headers["host"], "elsewhere.invalid");
}
