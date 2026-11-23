use tokio::io::AsyncReadExt as _;

pub(super) async fn ready() -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let mut stream =
            tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, 22)).await?;
        let mut prefix = [0; 8];
        stream.read_exact(&mut prefix).await?;
        Ok::<_, std::io::Error>(&prefix == b"SSH-2.0-" || &prefix == b"SSH-1.99")
    })
    .await
    .is_ok_and(|result| matches!(result, Ok(true)))
}
