use super::*;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio_rustls::{TlsAcceptor, TlsConnector};

fn enrollment(dir: &std::path::Path, key: u8) -> Issuer {
    let config = Config::parse(br#"{"hub":"https://hub.example","device":"00000000-0000-0000-0000-000000000009","deviceName":"synthetic"}"#).unwrap();
    Issuer::for_device(&[key; 32], &config, dir).unwrap()
}

async fn exchange(
    client: &Issuer,
    serving: &Issuer,
    client_ca: &Issuer,
    role: ExtendedKeyUsagePurpose,
) -> io::Result<[u8; 5]> {
    let (sender, receiver) = tokio::net::UnixStream::pair()?;
    let client = TlsConnector::from(Arc::new(client.client().unwrap()));
    let leaf = serving.leaf(role).unwrap();
    let accept = TlsAcceptor::from(Arc::new(server(&leaf, client_ca.0.der().clone()).unwrap()));
    let send = async move {
        let mut peer = client.connect(NAME.try_into().unwrap(), sender).await?;
        peer.write_all(b"hello").await?;
        peer.flush().await?;
        let mut ack = [0];
        peer.read_exact(&mut ack).await.map(|_| ())
    };
    let receive = async move {
        let mut peer = accept.accept(receiver).await?;
        let mut text = [0; 5];
        peer.read_exact(&mut text).await?;
        peer.write_all(b"!").await?;
        peer.flush().await?;
        Ok(text)
    };
    tokio::try_join!(send, receive).map(|((), text)| text)
}

#[tokio::test]
async fn only_the_enrolled_holder_and_runner_can_exchange_frames() {
    let dir = tempfile::tempdir().unwrap();
    let same = enrollment(dir.path(), 1);
    let restarted = enrollment(dir.path(), 1);
    let other = enrollment(dir.path(), 2);
    assert_eq!(
        exchange(
            &restarted,
            &same,
            &same,
            ExtendedKeyUsagePurpose::ServerAuth
        )
        .await
        .unwrap(),
        *b"hello"
    );
    assert!(
        exchange(&other, &same, &same, ExtendedKeyUsagePurpose::ServerAuth)
            .await
            .is_err()
    );
    assert!(
        exchange(&same, &same, &other, ExtendedKeyUsagePurpose::ServerAuth)
            .await
            .is_err()
    );
    assert!(
        exchange(&same, &same, &same, ExtendedKeyUsagePurpose::ClientAuth)
            .await
            .is_err()
    );
    let elsewhere = tempfile::tempdir().unwrap();
    assert!(
        exchange(
            &enrollment(elsewhere.path(), 1),
            &same,
            &same,
            ExtendedKeyUsagePurpose::ServerAuth
        )
        .await
        .is_err()
    );
}
