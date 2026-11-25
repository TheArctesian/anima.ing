use std::path::PathBuf;
use std::time::Duration;

use super::*;

#[tokio::test]
async fn tc7_w3_a_closed_client_cannot_enqueue_a_delayed_reply() {
    let (mut client, server) = UnixStream::pair().expect("a synthetic connection");
    let (send, mut calls) = mpsc::channel(1);
    let serving = tokio::spawn(serve(server, send, PathBuf::from("/fixture")));
    client
        .write_all(b"{\"id\":1,\"call\":\"version\"}\n")
        .await
        .expect("send the call");
    let pending = tokio::time::timeout(Duration::from_secs(8), calls.recv())
        .await
        .expect("the runner received the call")
        .expect("a pending call");
    assert_eq!(pending.client.with_open(|| 1), Some(1));
    drop(client);
    assert!(
        tokio::time::timeout(Duration::from_secs(8), calls.recv())
            .await
            .expect("the socket reader closed")
            .is_none()
    );
    assert!(
        pending
            .client
            .with_open(|| panic!("a delayed reply after client EOF"))
            .is_none()
    );
    drop(pending);
    serving.await.expect("the connection ended");
}
