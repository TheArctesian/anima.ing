use super::HubClient;
use agent_threads::managed::workspace::handoff::Receipt;
use bytes::Bytes;
use daemon_core::wire::DaemonToken;
use futures_util::{StreamExt as _, TryStreamExt as _, stream::BoxStream};
use http_body_util::{BodyExt as _, Empty, StreamBody, combinators::UnsyncBoxBody};
use hyper::body::Frame;
use hyper::http::{Method, Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use std::{io, time::Duration};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub(crate) type Stream = BoxStream<'static, io::Result<Bytes>>;
type Body = UnsyncBoxBody<Bytes, io::Error>;
const DEADLINE: Duration = Duration::from_mins(30);

pub(crate) struct Response {
    pub(super) status: StatusCode,
    pub(super) length: Option<u64>,
    pub(crate) bytes: Stream,
    _driver: Option<Driver>,
}

struct Driver(JoinHandle<hyper::Result<()>>);

impl Drop for Driver {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl HubClient {
    pub(super) async fn checkpoint(
        &self,
        request: Request<()>,
        stream: Option<Stream>,
    ) -> Option<Response> {
        tokio::time::timeout(DEADLINE, self.checkpoint_request(request, stream))
            .await
            .ok()
            .flatten()
    }

    async fn checkpoint_request(
        &self,
        request: Request<()>,
        stream: Option<Stream>,
    ) -> Option<Response> {
        if let Some(local) = &self.local {
            let (mut parts, ()) = request.into_parts();
            parts.headers.insert(
                header::HOST,
                hyper::http::HeaderValue::from_static("127.0.0.1"),
            );
            let peer = local.connect().await.ok()?;
            let (mut sender, connection) =
                hyper::client::conn::http1::handshake(TokioIo::new(peer))
                    .await
                    .ok()?;
            let driver = Driver(tokio::spawn(connection));
            let reply = sender
                .send_request(Request::from_parts(parts, body(stream)))
                .await
                .ok()?;
            let (parts, incoming) = reply.into_parts();
            return Some(Response {
                status: parts.status,
                length: length(&parts.headers),
                bytes: incoming
                    .into_data_stream()
                    .map_err(io::Error::other)
                    .boxed(),
                _driver: Some(driver),
            });
        }
        if self.base.scheme() != "https" {
            return None;
        }
        let url = self
            .base
            .join(request.uri().path().trim_start_matches('/'))
            .ok()?;
        let (parts, ()) = request.into_parts();
        let mut request = self
            .http
            .request(parts.method, url)
            .headers(parts.headers)
            .timeout(DEADLINE);
        if let Some(stream) = stream {
            request = request.body(reqwest::Body::wrap_stream(stream));
        }
        let response = request.send().await.ok()?;
        let status = response.status();
        let length = length(response.headers());
        Some(Response {
            status,
            length,
            bytes: response.bytes_stream().map_err(io::Error::other).boxed(),
            _driver: None,
        })
    }
}

impl Response {
    pub(super) async fn receipt(mut self) -> Option<Receipt> {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut result = Vec::new();
            while let Some(bytes) = self.bytes.try_next().await.ok()? {
                if result.len() + bytes.len() > 4096 {
                    return None;
                }
                result.extend(bytes);
            }
            serde_json::from_slice(&result).ok()
        })
        .await
        .ok()
        .flatten()
    }
}

pub(super) fn request(method: Method, transfer: Uuid, token: &DaemonToken) -> Request<()> {
    build(method, "checkpoint", transfer, token)
}

pub(super) fn control(operation: &str, transfer: Uuid, token: &DaemonToken) -> Request<()> {
    build(Method::POST, operation, transfer, token)
}

fn build(method: Method, operation: &str, transfer: Uuid, token: &DaemonToken) -> Request<()> {
    Request::builder()
        .uri(format!(
            "/daemon/v1/workspace-handoffs/{transfer}/{operation}"
        ))
        .method(method)
        .header(header::AUTHORIZATION, format!("Bearer {}", token.expose()))
        .body(())
        .expect("the path, headers and token have canonical syntax")
}

fn body(stream: Option<Stream>) -> Body {
    stream.map_or_else(
        || Empty::new().map_err(|never| match never {}).boxed_unsync(),
        |stream| StreamBody::new(stream.map_ok(Frame::data)).boxed_unsync(),
    )
}

fn length(headers: &hyper::http::HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

pub(super) fn single(bytes: Vec<u8>) -> Stream {
    futures_util::stream::once(async { Ok(Bytes::from(bytes)) }).boxed()
}
