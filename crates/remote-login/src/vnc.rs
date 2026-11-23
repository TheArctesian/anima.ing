//! Evidence of mandatory protocol authentication, not successful login or transport encryption.

use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

/// Unknown modes stay closed. In particular, x11vnc's unixpw screen cannot be proven by RFB None.
pub(super) async fn requires_authentication<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> bool {
    tokio::time::timeout(Duration::from_secs(2), offered(stream))
        .await
        .is_ok_and(|result| matches!(result, Ok(true)))
}

async fn offered<S: AsyncRead + AsyncWrite + Unpin>(stream: &mut S) -> std::io::Result<bool> {
    let mut banner = [0; 12];
    stream.read_exact(&mut banner).await?;
    // These are the dialects supported by Anima's gateway; Apple's 3.889 accepts a 3.8 reply.
    if &banner != b"RFB 003.008\n" && &banner != b"RFB 003.889\n" {
        return Ok(false);
    }
    stream.write_all(b"RFB 003.008\n").await?;
    let count = stream.read_u8().await?;
    if count == 0 {
        return Ok(false);
    }
    // The wire count is one byte, bounding both memory and the remaining read to 255 bytes.
    let mut methods = vec![0; usize::from(count)];
    stream.read_exact(&mut methods).await?;
    // Every offered path must require authentication, not merely the path we would prefer.
    // Apple-only companion types: RSA/SRP (33), Kerberos/GSSAPI (35), direct SRP (36).
    // Classification evidence and limits are recorded in HM-T41 of hosted-mode.md.
    let apple = &banner == b"RFB 003.889\n";
    Ok(methods.iter().any(|method| matches!(method, 2 | 30))
        && methods
            .iter()
            .all(|method| matches!(method, 2 | 30) || (apple && matches!(method, 33 | 35 | 36))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, duplex};

    async fn server(bytes: &[u8]) -> (DuplexStream, DuplexStream) {
        let (client, mut server) = duplex(512);
        server.write_all(bytes).await.unwrap();
        server.shutdown().await.unwrap();
        (client, server)
    }

    #[tokio::test]
    async fn an_rfb_banner_does_not_admit_an_anonymous_server() {
        for methods in [&[1][..], &[1, 2], &[30, 1], &[2, 19], &[0], &[255], &[]] {
            let bytes = [
                b"RFB 003.008\n".as_slice(),
                &[u8::try_from(methods.len()).unwrap()],
                methods,
            ]
            .concat();
            let (mut client, _server) = server(&bytes).await;
            assert!(!requires_authentication(&mut client).await, "{methods:?}");
        }
    }

    #[tokio::test]
    async fn only_mandatory_password_or_apple_authentication_is_admitted() {
        for (banner, methods) in [
            (b"RFB 003.008\n", vec![2]),
            (b"RFB 003.889\n", vec![30]),
            (b"RFB 003.889\n", vec![30, 33, 36, 35]),
            (b"RFB 003.889\n", vec![2, 30]),
            (b"RFB 003.008\n", vec![2; 255]),
        ] {
            let bytes = [
                banner.as_slice(),
                &[u8::try_from(methods.len()).unwrap()],
                &methods,
            ]
            .concat();
            let (mut client, mut server) = server(&bytes).await;
            assert!(requires_authentication(&mut client).await);
            let mut reply = [0; 12];
            server.read_exact(&mut reply).await.unwrap();
            assert_eq!(&reply, b"RFB 003.008\n");
            drop(client);
            let mut extra = Vec::new();
            server.read_to_end(&mut extra).await.unwrap();
            assert!(
                extra.is_empty(),
                "the probe never submits a credential or opens a session"
            );
        }
    }

    #[tokio::test]
    async fn apple_companions_need_the_apple_dialect_and_a_supported_authentication_path() {
        for (banner, methods) in [
            (b"RFB 003.008\n", vec![30, 33, 36, 35]),
            (b"RFB 003.889\n", vec![33, 36, 35]),
            (b"RFB 003.889\n", vec![30, 33, 36, 35, 1]),
            (b"RFB 003.889\n", vec![30, 33, 36, 35, 19]),
            (b"RFB 003.889\n", vec![30, 33, 36, 35, 255]),
        ] {
            let bytes = [
                banner.as_slice(),
                &[u8::try_from(methods.len()).unwrap()],
                &methods,
            ]
            .concat();
            let (mut client, _server) = server(&bytes).await;
            assert!(!requires_authentication(&mut client).await, "{methods:?}");
        }
    }

    #[tokio::test]
    async fn malformed_unsupported_and_truncated_exchanges_stay_closed() {
        for bytes in [
            &b"RFB 003.00x\n\x01\x02"[..],
            b"RFB 003.007\n\x01\x02",
            b"RFB 003.003\n\0\0\0\x02",
            b"RFB 004.008\n\x01\x02",
            b"RFB 003.999\n\x01\x02",
            b"RFB 003.008\r\x01\x02",
            b"RFB 003.008\n",
            b"RFB 003.008\n\x02\x02",
            b"RFB 003.008\n\xff\x02",
            b"RFB 003.",
        ] {
            let (mut client, _server) = server(bytes).await;
            assert!(!requires_authentication(&mut client).await, "{bytes:?}");
        }
    }

    #[tokio::test]
    async fn an_incomplete_security_offer_has_a_bounded_wait() {
        let (mut client, mut server) = duplex(256);
        server.write_all(b"RFB 003.008\n\x02\x02").await.unwrap();
        assert!(!requires_authentication(&mut client).await);
    }
}
