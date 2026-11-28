use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

pub(super) struct Listener {
    tcp: TcpListener,
    tls: TlsAcceptor,
    root: String,
}

impl Listener {
    pub(super) fn bind() -> Self {
        let tcp = bind_port();
        let address = tcp.local_addr().expect("the TLS hub address").ip();
        let tcp = TcpListener::from_std(tcp).expect("the K8 TCP listener");
        let (root, tls) = certificate(address.to_string());
        Self { tcp, tls, root }
    }

    pub(super) fn origin(&self) -> String {
        let address = self.tcp.local_addr().expect("the TLS hub address");
        format!("https://{}", address.ip())
    }

    pub(super) fn root(&self) -> &str {
        &self.root
    }
}

impl axum::serve::Listener for Listener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (tcp, address) = axum::serve::Listener::accept(&mut self.tcp).await;
            if let Ok(Ok(tls)) =
                tokio::time::timeout(Duration::from_secs(3), self.tls.accept(tcp)).await
            {
                return (tls, address);
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

fn bind_port() -> std::net::TcpListener {
    // Production K8 uses HTTPS on port 443. A separate loopback address lets fixtures run in parallel.
    for _ in 0..16 {
        let mut host = [0; 2];
        getrandom::fill(&mut host).expect("random loopback address");
        let address = Ipv4Addr::new(127, 43, host[0], host[1]);
        match std::net::TcpListener::bind((address, 443)) {
            Ok(tcp) => {
                tcp.set_nonblocking(true).expect("nonblocking TLS listener");
                return tcp;
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => (),
            Err(error) => panic!("cannot bind the K8 TLS hub at {address}:443: {error}"),
        }
    }
    panic!("could not find a free loopback address for the K8 TLS hub");
}

fn certificate(address: String) -> (String, TlsAcceptor) {
    let mut ca = CertificateParams::default();
    ca.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca_key = KeyPair::generate().expect("the fixture CA key");
    let root = ca.self_signed(&ca_key).expect("the fixture CA").pem();
    let mut leaf = CertificateParams::new(vec![address]).expect("the K8 server name");
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let key = KeyPair::generate().expect("the fixture leaf key");
    let leaf = leaf
        .signed_by(&key, &Issuer::new(ca, ca_key))
        .expect("the signed K8 server certificate");
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("TLS protocol versions")
    .with_no_client_auth()
    .with_single_cert(
        vec![leaf.der().clone()],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
    )
    .expect("the K8 server certificate and key");
    (root, TlsAcceptor::from(Arc::new(config)))
}
