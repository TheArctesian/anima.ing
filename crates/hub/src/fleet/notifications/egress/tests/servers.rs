mod mail;

use std::{
    net::SocketAddr,
    sync::{
        Arc, Mutex,
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
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::TlsAcceptor;

use mail::{GO_AHEAD, PLAIN, Script, implicit, smtp, starttls};

#[derive(Clone)]
pub(in crate::fleet::notifications) enum Behaviour {
    Http(Vec<u8>, Option<TlsAcceptor>),
    Endless(Option<TlsAcceptor>),
    /// Over TLS: waits, writes this head (blank line included) and a few body bytes, then stalls.
    Stalled(&'static [u8], Duration, TlsAcceptor),
    Silent,
    SmtpImplicit(TlsAcceptor),
    SmtpRefusingLogin(TlsAcceptor),
    /// Queuing the message takes this long, and QUIT gets no answer.
    SmtpSilentAtQuit(Duration, TlsAcceptor),
    SmtpPlain,
    SmtpStartTls(TlsAcceptor),
    /// A plaintext 554 in the same write as the 220, then the upgrade (a STARTTLS response injection).
    SmtpStartTlsInjecting(TlsAcceptor),
}

#[derive(Default)]
struct Seen {
    peers: Mutex<Vec<SocketAddr>>,
    ended: AtomicUsize,
    lines: Mutex<Vec<String>>,
    headers: Mutex<Vec<String>>,
    bodies: Mutex<Vec<Vec<u8>>>,
    names: Mutex<Vec<String>>,
    written: AtomicUsize,
}

pub(in crate::fleet::notifications) struct Server {
    pub(in crate::fleet::notifications) port: u16,
    seen: Arc<Seen>,
    sentinels: Mutex<Vec<SocketAddr>>,
}

impl Server {
    /// Waits until every earlier connection has ended: a sentinel connection is accepted after them.
    pub(in crate::fleet::notifications) async fn settle(&self) {
        let sentinel = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        let address = sentinel.local_addr().unwrap();
        drop(sentinel);
        self.sentinels.lock().unwrap().push(address);
        let seen = self.seen.clone();
        let done = async move {
            loop {
                let peers = seen.peers.lock().unwrap().clone();
                if peers.contains(&address) && seen.ended.load(Ordering::SeqCst) == peers.len() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(60), done)
            .await
            .expect("the test server settles");
    }

    /// The connections accepted, not counting [`Server::settle`]'s own.
    pub(in crate::fleet::notifications) fn connections(&self) -> usize {
        let sentinels = self.sentinels.lock().unwrap();
        let peers = self.seen.peers.lock().unwrap();
        peers
            .iter()
            .filter(|peer| !sentinels.contains(peer))
            .count()
    }

    pub(in crate::fleet::notifications) fn lines(&self) -> Vec<String> {
        self.seen.lines.lock().unwrap().clone()
    }

    pub(in crate::fleet::notifications) fn headers(&self) -> Vec<String> {
        self.seen.headers.lock().unwrap().clone()
    }

    pub(in crate::fleet::notifications) fn bodies(&self) -> Vec<Vec<u8>> {
        self.seen.bodies.lock().unwrap().clone()
    }

    /// The server name of each completed TLS handshake ("" when the client sent none).
    pub(in crate::fleet::notifications) fn server_names(&self) -> Vec<String> {
        self.seen.names.lock().unwrap().clone()
    }

    /// The body bytes an [`Behaviour::Endless`] server wrote before the client went.
    pub(in crate::fleet::notifications) fn written(&self) -> usize {
        self.seen.written.load(Ordering::SeqCst)
    }
}

pub(in crate::fleet::notifications) fn certificate(name: &str) -> (Vec<u8>, TlsAcceptor) {
    let key = rcgen::KeyPair::generate().unwrap();
    let params = rcgen::CertificateParams::new(vec![name.to_owned()]).unwrap();
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

pub(in crate::fleet::notifications) async fn serve(behaviour: Behaviour) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Seen::default());
    let recorder = seen.clone();
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
            recorder.peers.lock().unwrap().push(peer);
            let (seen, behaviour) = (recorder.clone(), behaviour.clone());
            tokio::spawn(async move {
                answer(stream, behaviour, &seen).await;
                seen.ended.fetch_add(1, Ordering::SeqCst);
            });
        }
    });
    Server {
        port,
        seen,
        sentinels: Mutex::new(Vec::new()),
    }
}

async fn answer(stream: TcpStream, behaviour: Behaviour, seen: &Seen) {
    match behaviour {
        Behaviour::Http(reply, acceptor) => web(stream, acceptor, Some(&reply), seen).await,
        Behaviour::Endless(acceptor) => web(stream, acceptor, None, seen).await,
        Behaviour::Stalled(head, wait, acceptor) => {
            if let Some(tls) = handshake(&acceptor, stream, seen).await {
                stalled(tls, (wait, head), seen).await;
            }
        }
        Behaviour::Silent => silent(stream).await,
        Behaviour::SmtpPlain => drop(smtp(stream, PLAIN, seen).await),
        Behaviour::SmtpImplicit(acceptor) => implicit(&acceptor, stream, PLAIN, seen).await,
        Behaviour::SmtpRefusingLogin(acceptor) => {
            let script = Script {
                auth: b"535 refused\r\n",
                ..PLAIN
            };
            implicit(&acceptor, stream, script, seen).await;
        }
        Behaviour::SmtpSilentAtQuit(queuing, acceptor) => {
            let script = Script {
                quit: None,
                queuing,
                ..PLAIN
            };
            implicit(&acceptor, stream, script, seen).await;
        }
        Behaviour::SmtpStartTls(acceptor) => starttls(&acceptor, stream, GO_AHEAD, seen).await,
        Behaviour::SmtpStartTlsInjecting(acceptor) => {
            let injected = b"220 go ahead\r\n554 injected before TLS\r\n";
            starttls(&acceptor, stream, injected, seen).await;
        }
    }
}

async fn web(stream: TcpStream, acceptor: Option<TlsAcceptor>, reply: Option<&[u8]>, seen: &Seen) {
    match acceptor {
        None => web_on(stream, reply, seen).await,
        Some(acceptor) => {
            if let Some(tls) = handshake(&acceptor, stream, seen).await {
                web_on(tls, reply, seen).await;
            }
        }
    }
}

async fn web_on<S: AsyncRead + AsyncWrite + Unpin>(stream: S, reply: Option<&[u8]>, seen: &Seen) {
    match reply {
        Some(reply) => http(stream, reply, seen).await,
        None => endless(stream, seen).await,
    }
}

async fn handshake(
    acceptor: &TlsAcceptor,
    stream: TcpStream,
    seen: &Seen,
) -> Option<tokio_rustls::server::TlsStream<TcpStream>> {
    let tls = acceptor.accept(stream).await.ok()?;
    let name = tls.get_ref().1.server_name().unwrap_or("").to_owned();
    seen.names.lock().unwrap().push(name);
    Some(tls)
}

async fn request<S: AsyncRead + Unpin>(stream: S, seen: &Seen) -> Option<BufReader<S>> {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    let mut first = true;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.ok()? == 0 {
            return None;
        }
        if line == "\r\n" {
            break;
        }
        let recorded = if first { &seen.lines } else { &seen.headers };
        recorded.lock().unwrap().push(line.trim_end().to_owned());
        first = false;
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await.ok()?;
    seen.bodies.lock().unwrap().push(body);
    Some(reader)
}

async fn http<S: AsyncRead + AsyncWrite + Unpin>(stream: S, reply: &[u8], seen: &Seen) {
    if let Some(mut reader) = request(stream, seen).await {
        let _ = reader.get_mut().write_all(reply).await;
        let _ = reader.get_mut().shutdown().await;
    }
}

async fn endless<S: AsyncRead + AsyncWrite + Unpin>(stream: S, seen: &Seen) {
    let Some(mut reader) = request(stream, seen).await else {
        return;
    };
    let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n";
    if reader.get_mut().write_all(head).await.is_err() {
        return;
    }
    let chunk = [b'x'; 1024];
    while reader.get_mut().write_all(&chunk).await.is_ok() {
        seen.written.fetch_add(chunk.len(), Ordering::SeqCst);
    }
}

async fn stalled<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    (wait, head): (Duration, &[u8]),
    seen: &Seen,
) {
    let Some(mut reader) = request(stream, seen).await else {
        return;
    };
    tokio::time::sleep(wait).await;
    let wrote = reader.get_mut().write_all(head).await.is_ok()
        && reader.get_mut().write_all(b"xx").await.is_ok()
        && reader.get_mut().flush().await.is_ok();
    if wrote {
        drain(reader).await;
    }
}

pub(super) async fn drain<S: AsyncRead + Unpin>(mut stream: S) {
    let mut sink = [0; 1024];
    while stream.read(&mut sink).await.is_ok_and(|read| read > 0) {}
}

async fn silent(stream: TcpStream) {
    drain(stream).await;
}
