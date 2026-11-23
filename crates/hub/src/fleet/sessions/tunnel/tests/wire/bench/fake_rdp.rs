//! An in-process RDP server on loopback, as the opening's certificate read meets it.

use std::{sync::Arc, time::Duration};

use rcgen::{CertificateParams, DnType, KeyPair};
use rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
    time::timeout,
};
use tokio_rustls::TlsAcceptor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Answer {
    /// `HYBRID`.
    Nla,
    /// `HYBRID_EX`.
    NlaExt,
    /// `SSL` only.
    Tls,
    Standard,
}

pub(in super::super) struct FakeRdp {
    pub(in super::super) port: u16,
    /// Computed here, not by the hub.
    pub(in super::super) fingerprint: String,
    visits: mpsc::UnboundedReceiver<()>,
    task: JoinHandle<()>,
}

impl FakeRdp {
    pub(in super::super) async fn was_visited(&mut self) -> bool {
        timeout(Duration::from_millis(300), self.visits.recv())
            .await
            .is_ok()
    }
}

impl Drop for FakeRdp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn fingerprint_of(der: &[u8]) -> String {
    let pairs: Vec<String> = Sha256::digest(der)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    pairs.join(":")
}

fn throwaway() -> (Vec<u8>, TlsAcceptor) {
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(vec!["desk.lan".to_owned()]).unwrap();
    params.distinguished_name.push(DnType::CommonName, "desk");
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

/// An X.224 Connection Confirm with an RDP Negotiation Response choosing `protocol`.
fn confirm(protocol: u32) -> Vec<u8> {
    let mut bytes = vec![3, 0, 0, 19, 14, 0xd0, 0, 0, 0x12, 0x34, 0, 2, 0, 8, 0];
    bytes.extend_from_slice(&protocol.to_le_bytes());
    bytes
}

async fn visit(mut stream: TcpStream, answer: Answer, acceptor: TlsAcceptor) {
    let mut request = [0; 19];
    if stream.read_exact(&mut request).await.is_err() {
        return;
    }
    let chosen = match answer {
        Answer::Nla => 0x2,
        Answer::NlaExt => 0x8,
        Answer::Tls => 0x1,
        Answer::Standard => 0,
    };
    if stream.write_all(&confirm(chosen)).await.is_err() || answer == Answer::Standard {
        return;
    }
    if let Ok(mut tls) = acceptor.accept(stream).await {
        let mut rest = Vec::new();
        let _ended = timeout(Duration::from_secs(5), tls.read_to_end(&mut rest)).await;
    }
}

pub(super) async fn fake_rdp(answer: Answer) -> FakeRdp {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (der, acceptor) = throwaway();
    let (sender, visits) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let _ = sender.send(());
            tokio::spawn(visit(stream, answer, acceptor.clone()));
        }
    });
    FakeRdp {
        port,
        fingerprint: fingerprint_of(&der),
        visits,
        task,
    }
}
