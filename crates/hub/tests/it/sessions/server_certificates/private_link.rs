//! Certificate discovery through the enrolled device's private dial transport.

use anima_hub::fleet::{sessions::Live, targets::AddressPolicy};
use serde_json::json;
use sqlx::PgPool;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _, copy_bidirectional},
    net::TcpStream,
};

use super::{
    result,
    servers::{Answer, Certificate, rdp},
    test_request,
};
use crate::{local_socket, sessions::host_keys::schema};

#[sqlx::test(migrations = "./migrations")]
async fn private_link_certificate_discovery_completes_tls_without_account_bytes(pool: PgPool) {
    let certificate = Certificate::new("PRIVATE-NODE");
    let mut server = rdp(Answer::Tls, &certificate).await;
    let folder = tempfile::tempdir().unwrap();
    local_socket::protect(folder.path());
    let path = folder.path().join("dial.sock");
    let listener = local_socket::Listener::bind(&path).unwrap();
    let port = server.port;
    let relay = tokio::spawn(async move {
        let mut stream = local_socket::accept(&listener).await.unwrap();
        let expected = format!("dial fd41:4e49:4d41::7 {port}\n");
        let mut request = vec![0; expected.len()];
        stream.read_exact(&mut request).await.unwrap();
        assert_eq!(request, expected.as_bytes());
        let mut device = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(b"ok\n").await.unwrap();
        // A probe may close without TLS close_notify, so only the observed device bytes matter.
        let _closed = copy_bidirectional(&mut stream, &mut device).await;
    });
    let live = Live::default().denying(AddressPolicy::default().reaching_link(Some(path)));
    let input = json!({
        "protocol": "RDP", "host": "fd41:4e49:4d41::7",
        "rdp": {"enabled": true, "port": port, "username": "synthetic-user"},
        "password": {"password": "synthetic-password"}
    });
    let test = result(schema(&pool, &live).execute(test_request(&input)).await);
    relay.await.unwrap();
    let visit = server.visit().await;
    assert_eq!(test["reachable"], true, "{test}");
    assert_eq!(test["authenticated"], false);
    assert_eq!(
        test["serverCertificate"]["fingerprintSha256"],
        certificate.fingerprint()
    );
    assert!(visit.handshake);
    assert!(
        visit.after.is_empty(),
        "discovery must not send credentials"
    );
}
