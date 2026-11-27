use super::{Certificate, Server, Visit, rest, serve};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

pub async fn vnc_tls(certificate: &Certificate, port: u16) -> Server {
    let acceptor = certificate.acceptor.clone();
    serve(
        move |mut stream| {
            let acceptor = acceptor.clone();
            async move {
                stream.write_all(b"RFB 003.008\n").await.unwrap();
                let mut version = [0; 12];
                stream.read_exact(&mut version).await.unwrap();
                assert_eq!(&version, b"RFB 003.008\n");
                stream.write_all(&[1, 19]).await.unwrap();
                assert_eq!(stream.read_u8().await.unwrap(), 19);
                stream.write_all(&[0, 2]).await.unwrap();
                assert_eq!(stream.read_u16().await.unwrap(), 2);
                stream.write_all(&[0, 1, 0, 0, 1, 6]).await.unwrap();
                assert_eq!(stream.read_u32().await.unwrap(), 262);
                stream.write_u8(1).await.unwrap();
                let tls = acceptor.accept(stream).await.unwrap();
                Visit {
                    request: version.to_vec(),
                    handshake: true,
                    after: rest(tls).await,
                }
            }
        },
        port,
    )
    .await
}
