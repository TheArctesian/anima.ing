//! Local test sites on 127.0.0.1 only; no keep-alive, so each HTTPS request is a new TLS handshake.

use std::convert::Infallible;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CONNECTION, CONTENT_TYPE, COOKIE, SET_COOKIE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::task::{JoinHandle, JoinSet};
use tokio_rustls::TlsAcceptor;

use crate::event::{Event, Events};
use crate::pages;
use crate::pin::Fingerprint;

const REPORT_LIMIT: usize = 16 * 1024;
const CONNECTIONS: usize = 64;

/// Dropping a site does not stop it; call `stop`.
pub struct Site {
    pub port: u16,
    pub fingerprint: Option<Fingerprint>,
    accept: JoinHandle<()>,
}

impl Site {
    pub async fn stop(self) {
        self.accept.abort();
        let _cancelled = self.accept.await;
    }
}

#[derive(Debug)]
pub enum SiteError {
    Io(io::Error),
    Certificate(String),
}

impl std::fmt::Display for SiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "listen: {}", e.kind()),
            Self::Certificate(e) => write!(f, "certificate: {e}"),
        }
    }
}

impl std::error::Error for SiteError {}

pub fn serve_http(events: Events) -> Result<Site, SiteError> {
    let listener = bind(0).map_err(SiteError::Io)?;
    let port = listener.local_addr().map_err(SiteError::Io)?.port();
    let accept = tokio::spawn(accept_loop(listener, None, "http".into(), events));
    Ok(Site {
        port,
        fingerprint: None,
        accept,
    })
}

pub fn serve_tls(port: u16, generation: u32, events: Events) -> Result<Site, SiteError> {
    let names = vec!["127.0.0.1".to_owned(), "localhost".to_owned()];
    let issued = rcgen::generate_simple_self_signed(names)
        .map_err(|e| SiteError::Certificate(e.to_string()))?;
    let leaf: CertificateDer<'static> = issued.cert.der().clone();
    let fingerprint = Fingerprint::of(&leaf);
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(issued.signing_key.serialize_der()));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(|b| b.with_no_client_auth().with_single_cert(vec![leaf], key))
        .map_err(|e| SiteError::Certificate(e.to_string()))?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = bind(port).map_err(SiteError::Io)?;
    let port = listener.local_addr().map_err(SiteError::Io)?.port();
    let name: Arc<str> = format!("tls-{generation}").into();
    let accept = tokio::spawn(accept_loop(listener, Some(acceptor), name, events));
    Ok(Site {
        port,
        fingerprint: Some(fingerprint),
        accept,
    })
}

/// Address reuse lets a restarted site get the same port.
fn bind(port: u16) -> io::Result<TcpListener> {
    let socket = TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
    socket.listen(32)
}

async fn accept_loop(
    listener: TcpListener,
    acceptor: Option<TlsAcceptor>,
    name: Arc<str>,
    events: Events,
) {
    // Owned here so aborting this task drops, and so aborts, every connection.
    let mut connections = JoinSet::new();
    loop {
        let Ok((stream, _peer)) = listener.accept().await else {
            eprintln!("od7: accept failed on {name}");
            continue;
        };
        while connections.try_join_next().is_some() {}
        if connections.len() >= CONNECTIONS {
            continue;
        }
        let (acceptor, name, events) = (acceptor.clone(), name.clone(), events.clone());
        connections.spawn(connection(stream, acceptor, name, events));
    }
}

async fn connection(
    stream: TcpStream,
    acceptor: Option<TlsAcceptor>,
    name: Arc<str>,
    events: Events,
) {
    let service = service_fn(move |request| answer(request, name.clone(), events.clone()));
    let http = http1::Builder::new().keep_alive(false).clone();
    let served = match acceptor {
        None => http.serve_connection(TokioIo::new(stream), service).await,
        Some(acceptor) => match acceptor.accept(stream).await {
            // A refused handshake is what check 3 expects after a mismatch.
            Err(_) => return,
            Ok(tls) => http.serve_connection(TokioIo::new(tls), service).await,
        },
    };
    if served.is_err() {
        eprintln!("od7: a connection ended early");
    }
}

async fn answer(
    request: Request<Incoming>,
    server: Arc<str>,
    events: Events,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = request.uri().path().to_owned();
    let query = request.uri().query().unwrap_or("").to_owned();
    if request.method() == Method::POST && path == "/report" {
        let name = pages::query_value(&query, "name").unwrap_or("").to_owned();
        let body = match Limited::new(request.into_body(), REPORT_LIMIT)
            .collect()
            .await
        {
            Ok(collected) => serde_json::from_slice(&collected.to_bytes()).unwrap_or_default(),
            Err(_) => return Ok(status(StatusCode::PAYLOAD_TOO_LARGE)),
        };
        events.send(Event::Report {
            server: server.to_string(),
            name,
            body,
        });
        return Ok(status(StatusCode::NO_CONTENT));
    }
    let cookie = request
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let Some(page) = pages::page(&path, &query) else {
        return Ok(status(StatusCode::NOT_FOUND));
    };
    events.send(Event::Served {
        server: server.to_string(),
        path,
        query,
        cookie,
    });
    let mut response = Response::new(Full::new(Bytes::from(page.html)));
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        "text/html; charset=utf-8".parse().expect("static header"),
    );
    headers.insert(CONNECTION, "close".parse().expect("static header"));
    if let Some(cookie) = page.set_cookie.and_then(|c| c.parse().ok()) {
        headers.insert(SET_COOKIE, cookie);
    }
    Ok(response)
}

fn status(code: StatusCode) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = code;
    response
}
