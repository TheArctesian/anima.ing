use super::vnc_device::{HEIGHT, WIDTH, hear};
use rustls::{
    ServerConfig,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest as _, Sha256};
use std::{io, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_rustls::TlsAcceptor;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Visit {
    Probe,
    Credentials(String, String),
}

pub(super) struct Device {
    pub(super) address: SocketAddr,
    pub(super) fingerprint: String,
    pub(super) visits: mpsc::UnboundedReceiver<Visit>,
    pub(super) keys: mpsc::UnboundedReceiver<(u32, bool)>,
}

fn certificate() -> (TlsAcceptor, String) {
    let made = rcgen::generate_simple_self_signed(vec!["synthetic-vnc".into()]).unwrap();
    let der = made.cert.der().clone();
    let pin = Sha256::digest(&der)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(made.signing_key.serialize_der()));
    let config =
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![der], key)
            .unwrap();
    (TlsAcceptor::from(Arc::new(config)), pin)
}

pub(super) async fn device() -> Device {
    let (acceptor, fingerprint) = certificate();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (visited, visits) = mpsc::unbounded_channel();
    let (heard, keys) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (acceptor, visited, heard) = (acceptor.clone(), visited.clone(), heard.clone());
            tokio::spawn(async move {
                let _closed = serve(stream, acceptor, visited, heard).await;
            });
        }
    });
    Device {
        address,
        fingerprint,
        visits,
        keys,
    }
}

async fn negotiate(stream: &mut TcpStream) -> io::Result<()> {
    stream.write_all(b"RFB 003.008\n").await?;
    let mut version = [0; 12];
    stream.read_exact(&mut version).await?;
    assert_eq!(&version, b"RFB 003.008\n");
    stream.write_all(&[1, 19]).await?;
    assert_eq!(stream.read_u8().await?, 19);
    stream.write_all(&[0, 2]).await?;
    assert_eq!(stream.read_u16().await?, 2);
    stream.write_all(&[0, 1, 0, 0, 1, 6]).await?;
    assert_eq!(stream.read_u32().await?, 262);
    stream.write_u8(1).await
}

async fn serve(
    mut stream: TcpStream,
    acceptor: TlsAcceptor,
    visited: mpsc::UnboundedSender<Visit>,
    heard: mpsc::UnboundedSender<(u32, bool)>,
) -> io::Result<()> {
    negotiate(&mut stream).await?;
    let mut stream = acceptor.accept(stream).await?;
    let Ok(first) = stream.read_u8().await else {
        visited.send(Visit::Probe).unwrap();
        return Ok(());
    };
    let mut length = [first, 0, 0, 0];
    stream.read_exact(&mut length[1..]).await?;
    let username_len = u32::from_be_bytes(length);
    let password_len = stream.read_u32().await?;
    assert!(username_len <= 4096 && password_len <= 4096);
    let mut username = vec![0; usize::try_from(username_len).unwrap()];
    let mut password = vec![0; usize::try_from(password_len).unwrap()];
    stream.read_exact(&mut username).await?;
    stream.read_exact(&mut password).await?;
    visited
        .send(Visit::Credentials(
            String::from_utf8(username).unwrap(),
            String::from_utf8(password).unwrap(),
        ))
        .unwrap();
    stream.write_u32(0).await?;
    stream.read_u8().await?;
    stream
        .write_all(&[WIDTH.to_be_bytes(), HEIGHT.to_be_bytes()].concat())
        .await?;
    stream.write_all(&[0; 20]).await?;
    loop {
        let kind = stream.read_u8().await?;
        hear(&mut stream, kind, &heard).await?;
    }
}
