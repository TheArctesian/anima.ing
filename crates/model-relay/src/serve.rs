//! The listener: each connection of an agent program, and for each request the checks before
//! anything goes to a provider (Section 7.3).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::header::{ORIGIN, UPGRADE};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode, Uri};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::task::JoinSet;

use crate::forward::forward;
use crate::headers::{passed, passed_on};
use crate::secrets::{SECRET_LENGTH, Secrets};
use crate::upstream::Upstreams;

/// The most connections at one time; one thread uses a few.
const MOST_CONNECTIONS: usize = 128;
/// The longest wait for the head of a request, so that an idle connection does not stay.
const HEAD_WAIT: Duration = Duration::from_secs(30);
/// The wait after a fault of the listener.
const ACCEPT_PAUSE: Duration = Duration::from_millis(50);

type Body = BoxBody<Bytes, hyper::Error>;

/// Accepts connections until the task is stopped. The connections are tasks of this task:
/// when it stops, each of them stops, also in the middle of an answer.
pub async fn accept(listener: TcpListener, secrets: Arc<Secrets>, upstreams: Arc<Upstreams>) {
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
        connections.spawn(async move {
            let service = service_fn(|request| answer(request, &secrets, &upstreams));
            // A connection that ends with a fault is the business of the program that made it.
            let _ended = hyper::server::conn::http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(HEAD_WAIT)
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
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
    let (given, rest) = split(path);
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
    let (mut parts, body) = request.into_parts();
    parts.uri = uri;
    parts.headers = passed_on(&parts.headers);
    let method = parts.method.clone();
    match forward(upstream, Request::from_parts(parts, body)).await {
        Ok(response) => {
            let (mut parts, body) = response.into_parts();
            parts.headers = passed(&parts.headers);
            tracing::info!(thread = %thread, %method, status = parts.status.as_u16(), "relay: passed");
            Ok(Response::from_parts(parts, body.boxed()))
        }
        Err(error) => {
            tracing::warn!(thread = %thread, %method, %error, "relay: the provider was not reached");
            Ok(refusal(
                StatusCode::BAD_GATEWAY,
                "the relay did not reach the provider",
            ))
        }
    }
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
    response
}
