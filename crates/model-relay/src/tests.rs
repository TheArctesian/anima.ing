//! The relay against a provider that the test plays on the loopback address: what passes
//! unchanged, what is refused before a provider is asked, and what never reaches a log
//! (Sections 7.3 and 7.4 of `docs/proposals/shortened-answers.md`).

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{HeaderMap, Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};

use crate::Relay;
use crate::upstream::{Trust, Upstream, Upstreams};

/// What the played provider got.
#[derive(Debug, Clone)]
struct Got {
    method: Method,
    path: String,
    headers: HeaderMap,
    body: Bytes,
}

type Seen = Arc<Mutex<Vec<Got>>>;

/// A provider on the loopback address. It answers 200 with the body `answer to ` and the body
/// it got; a path with `moved` gets a redirect to a different host.
async fn provider() -> (SocketAddr, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let address = listener.local_addr().expect("an address");
    let seen = Seen::default();
    let record = seen.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let record = record.clone();
            tokio::spawn(async move {
                let service = service_fn(|request| played(request, record.clone()));
                let _ended = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    (address, seen)
}

async fn played(
    request: Request<Incoming>,
    record: Seen,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let (parts, body) = request.into_parts();
    let body = body.collect().await.expect("a body").to_bytes();
    let path = parts.uri.path_and_query().expect("a path").to_string();
    let mut answer = Response::new(Full::new(Bytes::from(
        [b"answer to ", body.as_ref()].concat(),
    )));
    answer
        .headers_mut()
        .insert("x-provider", "yes".parse().expect("a value"));
    if path.contains("moved") {
        *answer.status_mut() = StatusCode::FOUND;
        let elsewhere = "http://elsewhere.invalid/x".parse().expect("a value");
        answer.headers_mut().insert("location", elsewhere);
    }
    record.lock().expect("the list").push(Got {
        method: parts.method,
        path,
        headers: parts.headers,
        body,
    });
    Ok(answer)
}

/// A relay whose three hosts are one played provider, each with the prefix of the real one.
async fn relay() -> (Relay, Seen) {
    let (address, seen) = provider().await;
    let host = |prefix| Upstream {
        host: address.ip().to_string(),
        port: address.port(),
        trust: Trust::Plain,
        prefix,
    };
    let upstreams = Upstreams {
        anthropic: host(""),
        openai: host("/v1"),
        chatgpt: host("/backend-api/codex"),
    };
    (Relay::start_with(upstreams).await.expect("a relay"), seen)
}

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

/// One request to `url` (`http://127.0.0.1:port/path`), as an agent program makes it; the
/// body of the answer is not read yet.
async fn send(
    method: Method,
    url: &str,
    headers: &[(&str, &str)],
    body: &'static str,
) -> Response<Incoming> {
    let rest = url.strip_prefix("http://").expect("a plain address");
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let stream = TcpStream::connect(authority).await.expect("a connection");
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .expect("HTTP");
    tokio::spawn(connection);
    let mut request = Request::builder()
        .method(method)
        .uri(if path.is_empty() { "/" } else { path })
        .header("host", authority);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let request = request
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .expect("a request");
    sender.send_request(request).await.expect("an answer")
}

/// As [`send`], with the whole body of the answer.
async fn call(method: Method, url: &str, headers: &[(&str, &str)], body: &'static str) -> Answer {
    let (parts, body) = send(method, url, headers, body).await.into_parts();
    let body = body.collect().await.expect("a body").to_bytes();
    Answer {
        status: parts.status,
        headers: parts.headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

fn seen(seen: &Seen) -> Vec<Got> {
    seen.lock().expect("the list").clone()
}

mod log;
mod passing;
mod refusals;
mod streams;
