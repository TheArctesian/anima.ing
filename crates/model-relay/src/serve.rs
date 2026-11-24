//! The listener: each connection of an agent program, and for each request the checks before
//! anything goes to a provider (Section 7.3).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::header::{CONNECTION, HeaderValue, ORIGIN, UPGRADE};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode, Uri};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;

use crate::forward::{ForwardError, forward};
use crate::headers::passed_on;
use crate::secrets::{SECRET_LENGTH, Secrets};
use crate::upstream::{Upstream, Upstreams};

/// The most connections at one time; one thread uses a few.
const MOST_CONNECTIONS: usize = 128;
/// The longest wait for the head of a request, so that an idle connection does not stay. It
/// also closes a connection that a program keeps open with no request for this time; the
/// program then opens a new one.
const HEAD_WAIT: Duration = Duration::from_secs(30);
/// The wait after a fault of the listener.
const ACCEPT_PAUSE: Duration = Duration::from_millis(50);

type Body = BoxBody<Bytes, hyper::Error>;

/// Accepts connections until the task is stopped. The connections are tasks of this task:
/// when it stops, each of them stops, also in the middle of an answer.
pub async fn accept(
    listener: TcpListener,
    secrets: Arc<Secrets>,
    upstreams: Arc<Upstreams>,
    tls: Option<TlsAcceptor>,
) {
    let mut connections = JoinSet::new();
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            // A fault such as "too many open files" would come again at once.
            tokio::time::sleep(ACCEPT_PAUSE).await;
            continue;
        };
        while connections.try_join_next().is_some() {}
        // Above the limit the connection is closed at once; the listener never waits.
        if !peer.ip().is_loopback() || connections.len() >= MOST_CONNECTIONS {
            continue;
        }
        let (secrets, upstreams) = (secrets.clone(), upstreams.clone());
        let tls = tls.clone();
        connections.spawn(async move {
            if let Some(tls) = tls {
                if let Ok(Ok(stream)) = tokio::time::timeout(HEAD_WAIT, tls.accept(stream)).await {
                    connection(stream, secrets, upstreams).await;
                }
            } else {
                connection(stream, secrets, upstreams).await;
            }
        });
    }
}

async fn connection(
    stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    secrets: Arc<Secrets>,
    upstreams: Arc<Upstreams>,
) {
    let service = service_fn(|request| answer(request, &secrets, &upstreams));
    // A connection that ends with a fault is the business of the program that made it.
    let _ended = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(HEAD_WAIT)
        .serve_connection(TokioIo::new(stream), service)
        .await;
}

/// The answer to one request: a refusal of the relay, or the answer of the provider.
async fn answer(
    request: Request<Incoming>,
    secrets: &Secrets,
    upstreams: &Upstreams,
) -> Result<Response<Body>, Infallible> {
    // A browser sends `origin` with a request of a page; an agent program never does.
    if request.headers().contains_key(ORIGIN) {
        return Ok(refusal(StatusCode::FORBIDDEN, "no request of a web page"));
    }
    let path = request
        .uri()
        .path_and_query()
        .map_or("", |path| path.as_str());
    let (given, rest) = if request.headers().contains_key("x-anima-relay") {
        let values = request.headers().get_all("x-anima-relay");
        if values.iter().count() != 1 {
            return Ok(refusal(StatusCode::NOT_FOUND, "not found"));
        }
        (
            values
                .iter()
                .next()
                .and_then(|value| value.to_str().ok())
                .unwrap_or(""),
            path,
        )
    } else {
        split(path)
    };
    let Some((thread, program)) = secrets.thread_of(given) else {
        return Ok(refusal(StatusCode::NOT_FOUND, "not found"));
    };
    if request.headers().contains_key(UPGRADE) {
        tracing::info!(thread = %thread, "relay: an upgrade of the connection was refused");
        return Ok(refusal(
            StatusCode::NOT_IMPLEMENTED,
            "the relay passes no upgraded connection",
        ));
    }
    let target = upstreams.target(program, request.headers(), rest);
    let Some((upstream, path)) = target else {
        return Ok(refusal(StatusCode::NOT_FOUND, "not a path of this program"));
    };
    let Ok(uri) = path.parse::<Uri>() else {
        return Ok(refusal(StatusCode::BAD_REQUEST, "not a path"));
    };
    Ok(passed(&thread, upstream, request, uri).await)
}

/// The answer of the provider to one request of a thread, or 502 when it was not reached.
async fn passed(
    thread: &str,
    upstream: &Upstream,
    request: Request<Incoming>,
    uri: Uri,
) -> Response<Body> {
    let (mut parts, body) = request.into_parts();
    parts.uri = uri;
    parts.headers = passed_on(&parts.headers);
    let method = parts.method.clone();
    match forward(upstream, Request::from_parts(parts, body)).await {
        Ok(response) => {
            let (mut parts, body) = response.into_parts();
            parts.headers = passed_on(&parts.headers);
            log_passed(thread, &method, parts.status);
            Response::from_parts(parts, body.boxed())
        }
        Err(error) => {
            log_not_reached(thread, &method, &error);
            refusal(
                StatusCode::BAD_GATEWAY,
                "the relay did not reach the provider",
            )
        }
    }
}

/// The log line of one passed request: the thread, the method and the status only.
fn log_passed(thread: &str, method: &Method, status: StatusCode) {
    tracing::info!(thread = %thread, %method, status = status.as_u16(), "relay: passed");
}

/// The log line of one request whose provider was not reached.
fn log_not_reached(thread: &str, method: &Method, error: &ForwardError) {
    tracing::warn!(thread = %thread, %method, %error, "relay: the provider was not reached");
}

/// The first part of a path, when it has the length of a secret, and what follows it.
fn split(path: &str) -> (&str, &str) {
    let path = path.strip_prefix('/').unwrap_or(path);
    let end = path.find(['/', '?']).unwrap_or(path.len());
    if end == SECRET_LENGTH {
        path.split_at(end)
    } else {
        ("", "")
    }
}

fn refusal(status: StatusCode, sentence: &'static str) -> Response<Body> {
    let body = Full::new(Bytes::from_static(sentence.as_bytes()))
        .map_err(|never: Infallible| match never {})
        .boxed();
    let mut response = Response::new(body);
    *response.status_mut() = status;
    // A caller that was refused does not keep its connection, and so no place of the limit.
    let close = HeaderValue::from_static("close");
    response.headers_mut().insert(CONNECTION, close);
    response
}
