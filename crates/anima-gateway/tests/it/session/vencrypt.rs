//! Pinned `X509Plain` over the real hub/gateway and TCP/TLS session boundaries.
use super::{
    draw_frame,
    hub_side::{session, values},
};
use crate::{drawn::Drawn, vnc::rfb};
use anima_gateway::session::{Ended, Settings};
use guac_protocol::Instruction;
use rustls::{
    ServerConfig,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest as _, Sha256};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_rustls::TlsAcceptor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    Accepted,
    ChangedPin,
    NoPin,
    PlainOnly,
    AnonymousOnly,
}

fn certificate() -> (Arc<ServerConfig>, String) {
    let made =
        rcgen::generate_simple_self_signed(vec!["synthetic-vnc".into()]).expect("certificate");
    let der = made.cert.der().clone();
    let pin = format!(
        "sha256:{}",
        Sha256::digest(&der)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(":")
    );
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(made.signing_key.serialize_der()));
    let config =
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(vec![der], key)
            .expect("TLS");
    (Arc::new(config), pin)
}

#[tokio::test]
async fn pinned_vnc_authenticates_over_tls_and_never_downgrades_or_sends_credentials_to_another_leaf()
 {
    for case in [
        Case::Accepted,
        Case::ChangedPin,
        Case::NoPin,
        Case::PlainOnly,
        Case::AnonymousOnly,
    ] {
        let (config, pin) = certificate();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let device = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("client");
            timeout(Duration::from_secs(5), serve(stream, config, case))
                .await
                .expect("bounded device")
                .expect("protocol")
        });
        let wrong = format!("sha256:{}", ["00"; 32].join(":"));
        let pin = match case {
            Case::ChangedPin => wrong.as_str(),
            Case::NoPin => "",
            _ => &pin,
        };
        let (mut hub, served) = session(Settings::default());
        hub.open(
            "vnc",
            values(
                address,
                &[
                    ("username", "synthetic-user"),
                    ("password", "synthetic-password"),
                    ("cert-fingerprints", pin),
                ],
            ),
        )
        .await
        .expect("ready");
        if case == Case::Accepted {
            let mut drawn = Drawn::default();
            draw_frame(&mut hub, &mut drawn).await;
            assert_eq!(drawn.rgb, [20, 40, 60]);
            hub.send(&Instruction::new("key", ["65", "1"])).await;
        } else {
            assert_eq!(hub.next_of("error").await.args[1], "515");
        }
        let seen = device.await.expect("device task");
        let ended = served.await.expect("gateway task");
        if case == Case::Accepted {
            assert_eq!(
                seen,
                Some(("synthetic-user".into(), "synthetic-password".into()))
            );
            assert_eq!(ended, Ended::ByDevice);
        } else {
            assert_eq!(seen, None, "{case:?}: no credential bytes");
            assert!(matches!(ended, Ended::NoSignInMethod | Ended::SignInFailed));
        }
    }
}

async fn serve(
    mut stream: TcpStream,
    config: Arc<ServerConfig>,
    case: Case,
) -> std::io::Result<Option<(String, String)>> {
    if !offer(&mut stream, case).await? || !subtype(&mut stream, case).await? {
        return Ok(None);
    }
    let mut tls = TlsAcceptor::from(config).accept(stream).await?;
    if case == Case::ChangedPin {
        assert!(tls.read_u8().await.is_err());
        return Ok(None);
    }
    let account = credentials(&mut tls).await?;
    framebuffer(&mut tls).await?;
    Ok(Some(account))
}

async fn offer(stream: &mut TcpStream, case: Case) -> std::io::Result<bool> {
    stream.write_all(b"RFB 003.008\n").await?;
    stream.read_exact(&mut [0; 12]).await?;
    stream
        .write_all(&[1, if case == Case::PlainOnly { 2 } else { 19 }])
        .await?;
    if matches!(case, Case::PlainOnly | Case::NoPin) {
        assert_eq!(stream.read(&mut [0; 1]).await?, 0);
        return Ok(false);
    }
    Ok(true)
}

async fn subtype(stream: &mut TcpStream, case: Case) -> std::io::Result<bool> {
    assert_eq!(stream.read_u8().await?, 19);
    stream.write_all(&[0, 2]).await?;
    assert_eq!(stream.read_u16().await?, 2);
    stream.write_all(&[0, 1]).await?;
    stream
        .write_u32(if case == Case::AnonymousOnly {
            259
        } else {
            262
        })
        .await?;
    if case == Case::AnonymousOnly {
        assert_eq!(stream.read(&mut [0; 1]).await?, 0);
        return Ok(false);
    }
    assert_eq!(stream.read_u32().await?, 262);
    stream.write_u8(1).await?;
    Ok(true)
}

async fn credentials(
    tls: &mut tokio_rustls::server::TlsStream<TcpStream>,
) -> std::io::Result<(String, String)> {
    let username = tls.read_u32().await? as usize;
    let password = tls.read_u32().await? as usize;
    assert!(username < 100 && password < 100);
    let mut account = vec![0; username + password];
    tls.read_exact(&mut account).await?;
    let user = String::from_utf8(account[..username].to_vec()).expect("username");
    let pass = String::from_utf8(account[username..].to_vec()).expect("password");
    Ok((user, pass))
}

async fn initialise(tls: &mut tokio_rustls::server::TlsStream<TcpStream>) -> std::io::Result<()> {
    tls.write_u32(0).await?;
    assert_eq!(tls.read_u8().await?, 1);
    tls.write_all(&[0, 1, 0, 1]).await?;
    tls.write_all(&[0; 20]).await?;
    tls.read_exact(&mut [0; 20]).await?;
    assert_eq!(tls.read_u8().await?, 2);
    tls.read_u8().await?;
    let count = usize::from(tls.read_u16().await?);
    assert!(count < 50);
    tls.read_exact(&mut vec![0; count * 4]).await?;
    tls.read_exact(&mut [0; 10]).await?;
    Ok(())
}

async fn framebuffer(tls: &mut tokio_rustls::server::TlsStream<TcpStream>) -> std::io::Result<()> {
    initialise(tls).await?;
    tls.write_all(&rfb::update(&[rfb::raw(
        anima_gateway::screen::Area {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        &[20, 40, 60],
    )]))
    .await?;
    // The next framebuffer request precedes the user's key.
    tls.read_exact(&mut [0; 10]).await?;
    let mut key = [0; 8];
    tls.read_exact(&mut key).await?;
    assert_eq!(key, [4, 1, 0, 0, 0, 0, 0, 65]);
    Ok(())
}
