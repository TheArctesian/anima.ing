use super::{LocalProtocol, protocol_answer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _, duplex};

#[tokio::test]
async fn appliance_checks_require_the_expected_protocol_or_security_exchange() {
    let cases: [(LocalProtocol, &[u8], bool); 8] = [
        (LocalProtocol::Ssh, b"SSH-2.0-OpenSSH\r\n", true),
        (LocalProtocol::Ssh, b"HTTP/1.1 200 OK\r\n", false),
        (
            LocalProtocol::Rdp,
            b"\x03\x00\x00\x13\x0e\xd0\x00\x00\x00\x00\x00\x02\x00\x08\x00\x01\x00\x00\x00",
            true,
        ),
        (
            LocalProtocol::Rdp,
            b"\x03\x00\x00\x0b\x06\xd0\x00\x00\x00\x00\x00",
            false,
        ),
        (LocalProtocol::Rdp, b"RFB 003.008\n", false),
        (
            LocalProtocol::Vnc,
            b"RFB 003.008\n\x01\x13\x00\x02\x00\x01\x00\x00\x01\x06",
            true,
        ),
        (
            LocalProtocol::Vnc,
            b"RFB 003.008\n\x02\x13\x02\x00\x02\x00\x01\x00\x00\x01\x06",
            false,
        ),
        (
            LocalProtocol::Vnc,
            b"RFB 003.008\n\x01\x13\x00\x02\x00\x02\x00\x00\x01\x06\x00\x00\x01\x03",
            false,
        ),
    ];
    for (protocol, reply, expected) in cases {
        let (mut client, mut server) = duplex(128);
        server.write_all(reply).await.unwrap();
        server.shutdown().await.unwrap();
        assert_eq!(
            protocol_answer(protocol, &mut client).await,
            expected,
            "{reply:?}"
        );
        if expected && matches!(protocol, LocalProtocol::Vnc) {
            drop(client);
            let mut sent = Vec::new();
            server.read_to_end(&mut sent).await.unwrap();
            assert_eq!(sent, b"RFB 003.008\n\x13\x00\x02");
        }
    }
}
