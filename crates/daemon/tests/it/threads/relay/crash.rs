//! A surviving child cannot send HTTP credentials to a replacement listener.

use super::super::WAIT;
use super::{Holder, Outcome, Program, collect, relayed, status};
use std::os::unix::fs::PermissionsExt as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

fn public_certificate(ca: &str) -> Vec<u8> {
    let certificate = std::fs::read(ca).expect("public certificate");
    assert_eq!(
        std::fs::metadata(ca).expect("file").permissions().mode() & 0o777,
        0o600
    );
    let directory = std::path::Path::new(ca).parent().expect("private folder");
    assert_eq!(
        std::fs::metadata(directory)
            .expect("folder")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert!(!String::from_utf8_lossy(&certificate).contains("PRIVATE KEY"));
    certificate
}

#[tokio::test]
async fn th118_a_rebound_port_gets_no_http_credential_after_holder_death() {
    let collector = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("collector");
    let target = collector.local_addr().expect("address");
    let receiving = tokio::spawn(collect(collector));
    let script = format!(
        "/usr/bin/curl --silent --noproxy '*' --data-binary \"$ANTHROPIC_BASE_URL\n$NODE_EXTRA_CA_CERTS\" http://{target}/; read line; /usr/bin/curl --silent --noproxy '*' --max-time 5 --retry 2 --retry-connrefused --retry-delay 1 --cacert \"$NODE_EXTRA_CA_CERTS\" -H 'Authorization: Bearer synthetic-credential' \"$ANTHROPIC_BASE_URL/v1/messages\""
    );
    let mut holder = Holder::start(0);
    let mut connection = holder.link().await;
    let id = connection
        .send(relayed("crash", &script, Program::ClaudeCode))
        .await;
    let Outcome::Started { pid } = connection.answer(id).await else {
        panic!("child started");
    };
    let received = tokio::time::timeout(WAIT, receiving)
        .await
        .expect("in time")
        .expect("URL");
    let (address, ca) = received.split_once('\n').expect("URL and certificate");
    let certificate = public_certificate(ca);
    assert_eq!(
        status(address, &certificate).await,
        501,
        "real holder authenticated"
    );
    let authority = address.split('/').nth(2).expect("loopback authority");
    holder.child.kill().expect("abrupt holder death");
    holder.child.wait().expect("holder reaped");
    let attacker = tokio::net::TcpListener::bind(authority)
        .await
        .expect("rebound port");
    let (mut socket, _) = tokio::time::timeout(WAIT, attacker.accept())
        .await
        .expect("retry")
        .expect("connection");
    let mut bytes = vec![0; 4096];
    let count = socket.read(&mut bytes).await.expect("TLS client hello");
    bytes.truncate(count);
    assert_eq!(bytes.first(), Some(&22), "TLS handshake, not HTTP");
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("synthetic-credential"));
    assert!(!text.contains(address.split('/').nth(3).expect("bearer")));
    socket
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
        .await
        .expect("false server");
    drop(socket);
    // The crash leaves only a public certificate. Remove this fixture after its curl exits.
    child_ended(pid).await;
    std::fs::remove_dir_all(std::path::Path::new(ca).parent().expect("folder"))
        .expect("public test certificate cleanup");
}

async fn child_ended(pid: u32) {
    tokio::time::timeout(WAIT, async {
        while anima_daemon::threads::tree::started(pid).await.is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("synthetic child ended");
}
