//! One request sent to a provider and its answer taken as a stream. A new connection for each
//! request; no redirect is followed, because nothing here reads a `location`.

use std::io;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use hyper::body::Incoming;
use hyper::header::{HOST, HeaderValue};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::upstream::{Trust, Upstream};

/// The longest wait for the connection to a provider. An answer has no limit: it is a stream.
const CONNECT_WAIT: Duration = Duration::from_secs(15);
/// The longest wait for the head of an answer.
const HEAD_WAIT: Duration = Duration::from_mins(20);

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("no connection to the provider: {0}")]
    Connect(#[from] io::Error),
    #[error("the provider did not answer in time")]
    Slow,
    #[error("the provider did not speak HTTP: {0}")]
    Http(#[from] hyper::Error),
    #[error("TLS could not be set up")]
    Tls,
}

/// Sends `request`, whose path is the path at the provider, and gives the answer. The body of
/// the request and the body of the answer are not read here: they pass as streams.
pub async fn forward(
    upstream: &Upstream,
    mut request: Request<Incoming>,
) -> Result<Response<Incoming>, ForwardError> {
    let host = HeaderValue::from_str(&upstream.host).map_err(|_| ForwardError::Tls)?;
    request.headers_mut().insert(HOST, host);
    // One wait for the connection, TLS and the start of HTTP: a host that accepts and then
    // says nothing must not hold the request.
    let sender = tokio::time::timeout(CONNECT_WAIT, connect(upstream))
        .await
        .map_err(|_| ForwardError::Slow)??;
    send(sender, request).await
}

type Sender = hyper::client::conn::http1::SendRequest<Incoming>;

async fn connect(upstream: &Upstream) -> Result<Sender, ForwardError> {
    let stream = TcpStream::connect((upstream.host.as_str(), upstream.port)).await?;
    let config = match &upstream.trust {
        Trust::WebRoots => web_roots()?,
        #[cfg(test)]
        Trust::Root(certificate) => {
            let mut roots = rustls::RootCertStore::empty();
            roots
                .add(certificate.clone().into())
                .map_err(|_| ForwardError::Tls)?;
            tls_config(roots).ok_or(ForwardError::Tls)?
        }
        #[cfg(test)]
        Trust::Plain => return handshake(stream).await,
    };
    let name = ServerName::try_from(upstream.host.clone()).map_err(|_| ForwardError::Tls)?;
    let secure = tokio_rustls::TlsConnector::from(config)
        .connect(name, stream)
        .await?;
    handshake(secure).await
}

async fn handshake<S>(stream: S) -> Result<Sender, ForwardError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
    // The connection lives until the answer has passed; its fault ends the stream of the answer.
    tokio::spawn(connection);
    Ok(sender)
}

/// Sends the request and waits for the head of the answer. A model can think for minutes
/// before its first byte, so the wait is long.
async fn send(
    mut sender: Sender,
    request: Request<Incoming>,
) -> Result<Response<Incoming>, ForwardError> {
    tokio::time::timeout(HEAD_WAIT, sender.send_request(request))
        .await
        .map_err(|_| ForwardError::Slow)?
        .map_err(ForwardError::Http)
}

/// The TLS setup with the public roots, made one time.
fn web_roots() -> Result<Arc<rustls::ClientConfig>, ForwardError> {
    static CONFIG: OnceLock<Option<Arc<rustls::ClientConfig>>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            tls_config(rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            })
        })
        .clone()
        .ok_or(ForwardError::Tls)
}

/// rustls with ring and these roots, HTTP/1.1 only.
fn tls_config(roots: rustls::RootCertStore) -> Option<Arc<rustls::ClientConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Some(Arc::new(config))
}
