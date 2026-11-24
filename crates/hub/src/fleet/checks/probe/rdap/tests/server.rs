//! A local HTTPS server for the RDAP tests; `pinned.invalid` never resolves (RFC 6761).

use std::{
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::TlsAcceptor;

/// The bytes a server writes for a request path; the second argument is its port.
pub(in crate::fleet::checks) type Answer = Arc<dyn Fn(&str, u16) -> Vec<u8> + Send + Sync>;

#[derive(Default)]
struct Seen {
    peers: Mutex<Vec<SocketAddr>>,
    lines: Mutex<Vec<String>>,
}

pub(in crate::fleet::checks) struct Server {
    address: SocketAddr,
    pub(in crate::fleet::checks) port: u16,
    pub(in crate::fleet::checks) root: Vec<u8>,
    seen: Arc<Seen>,
    sentinels: AtomicUsize,
}

impl Server {
    /// The accept loop takes connections in order, so once a sentinel shows, every earlier one has.
    pub(in crate::fleet::checks) async fn connections(&self) -> usize {
        let sentinel = TcpStream::connect(self.address).await.unwrap();
        let mine = sentinel.local_addr().unwrap();
        drop(sentinel);
        let shown = async {
            while !self.peers().contains(&mine) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(30), shown)
            .await
            .unwrap();
        let sentinels = self.sentinels.fetch_add(1, Ordering::SeqCst) + 1;
        self.peers().len() - sentinels
    }

    fn peers(&self) -> Vec<SocketAddr> {
        let peers = self.seen.peers.lock();
        peers.unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub(in crate::fleet::checks) fn lines(&self) -> Vec<String> {
        let lines = self.seen.lines.lock();
        lines.unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub(in crate::fleet::checks) fn requests_for(&self, suffix: &str) -> usize {
        let path_ends = |line: &String| {
            line.split(' ')
                .nth(1)
                .is_some_and(|path| path.ends_with(suffix))
        };
        self.lines().iter().filter(|line| path_ends(line)).count()
    }

    pub(in crate::fleet::checks) fn url(&self, path: &str) -> reqwest::Url {
        reqwest::Url::parse(&format!("https://localhost:{}{path}", self.port)).unwrap()
    }
}

pub(in crate::fleet::checks) fn reply(status: &str, body: &[u8]) -> Vec<u8> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/rdap+json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    [head.as_bytes(), body].concat()
}

pub(in crate::fleet::checks) fn redirect(location: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}

fn acceptor() -> (Vec<u8>, TlsAcceptor) {
    let key = rcgen::KeyPair::generate().unwrap();
    let params = rcgen::CertificateParams::new(vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "pinned.invalid".to_owned(),
    ])
    .unwrap();
    let der = params.self_signed(&key).unwrap().der().to_vec();
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key.serialize_der()));
    let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![der.clone().into()], key)
        .unwrap();
    (der, TlsAcceptor::from(Arc::new(config)))
}

pub(in crate::fleet::checks) async fn serve_on(ip: IpAddr, answer: Answer) -> Server {
    let listener = TcpListener::bind(SocketAddr::from((ip, 0))).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (root, acceptor) = acceptor();
    let seen = Arc::new(Seen::default());
    let recorder = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
            recorder
                .peers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(peer);
            let (acceptor, answer) = (acceptor.clone(), Arc::clone(&answer));
            let seen = Arc::clone(&recorder);
            tokio::spawn(async move {
                if let Ok(tls) = acceptor.accept(stream).await {
                    respond(tls, &answer, address.port(), &seen).await;
                }
            });
        }
    });
    Server {
        address,
        port: address.port(),
        root,
        seen,
        sentinels: AtomicUsize::new(0),
    }
}

pub(in crate::fleet::checks) async fn serve(
    answer: impl Fn(&str, u16) -> Vec<u8> + Send + Sync + 'static,
) -> Server {
    serve_on(IpAddr::from([127, 0, 0, 1]), Arc::new(answer)).await
}

async fn respond<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    answer: &Answer,
    port: u16,
    seen: &Seen,
) {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.unwrap_or(0) == 0 {
        return;
    }
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
    }
    let request_line = request_line.trim_end().to_owned();
    let path = request_line
        .split(' ')
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    seen.lines
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request_line);
    let _ = reader.get_mut().write_all(&answer(&path, port)).await;
    let _ = reader.get_mut().shutdown().await;
}
