use super::*;
use axum::{Router, routing::get};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message as ClientFrame;

#[tokio::test]
async fn forwards_bytes_in_both_directions_after_a_tcp_half_close() {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Router::new().route(
        "/",
        get(|upgrade: WebSocketUpgrade| async move {
            upgrade.on_upgrade(|socket| async move {
                let (reached, mut server) = tokio::io::duplex(64);
                let service = async {
                    let mut bytes = Vec::new();
                    server.read_to_end(&mut bytes).await.unwrap();
                    assert_eq!(bytes, b"development request");
                    server.write_all(b"development response").await.unwrap();
                    server.shutdown().await.unwrap();
                };
                let (result, ()) = tokio::join!(relay(socket, reached), service);
                result.unwrap();
            })
        }),
    );
    let server = tokio::spawn(async { axum::serve(listener, router).await.unwrap() });
    let exchange = async {
        let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/"))
            .await
            .unwrap();
        client
            .send(ClientFrame::Binary(Bytes::from_static(
                b"development request",
            )))
            .await
            .unwrap();
        client
            .send(ClientFrame::Binary(Bytes::new()))
            .await
            .unwrap();
        assert_eq!(
            client.next().await.unwrap().unwrap(),
            ClientFrame::Binary(Bytes::from_static(b"development response"))
        );
        assert_eq!(
            client.next().await.unwrap().unwrap(),
            ClientFrame::Binary(Bytes::new())
        );
    };
    let result = tokio::time::timeout(Duration::from_secs(5), exchange).await;
    server.abort();
    result.unwrap();
}
