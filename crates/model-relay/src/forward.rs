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

use crate::upstream::Upstream;

/// The longest wait for the connection to a provider. An answer has no limit: it is a stream.
const CONNECT_WAIT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("no connection to the provider: {0}")]
    Connect(#[from] io::Error),
    #[error("no connection to the provider in {} seconds", CONNECT_WAIT.as_secs())]
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
    let connecting = TcpStream::connect((upstream.host.as_str(), upstream.port));
    let stream = tokio::time::timeout(CONNECT_WAIT, connecting)
        .await
        .map_err(|_| ForwardError::Slow)??;
    if upstream.tls {
        let name = ServerName::try_from(upstream.host.clone()).map_err(|_| ForwardError::Tls)?;
        let secure = tokio_rustls::TlsConnector::from(tls_config()?)
            .connect(name, stream)
            .await?;
        send(secure, request).await
    } else {
        send(stream, request).await
    }
}

async fn send<S>(stream: S, request: Request<Incoming>) -> Result<Response<Incoming>, ForwardError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
    // The connection lives until the answer has passed; its fault ends the stream of the answer.
    tokio::spawn(connection);
    Ok(sender.send_request(request).await?)
}

/// rustls with ring and the web PKI roots, HTTP/1.1 only.
fn tls_config() -> Result<Arc<rustls::ClientConfig>, ForwardError> {
    static CONFIG: OnceLock<Option<Arc<rustls::ClientConfig>>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            };
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .ok()?
                .with_root_certificates(roots)
                .with_no_client_auth();
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            Some(Arc::new(config))
        })
        .clone()
        .ok_or(ForwardError::Tls)
}
