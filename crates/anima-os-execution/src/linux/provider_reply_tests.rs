use super::*;
use std::time::Duration;

#[tokio::test]
async fn claude_forwards_one_response_and_keeps_the_session_open_until_disconnect() {
    let (mut human, input) = tokio::io::duplex(4096);
    let (mut output, mut worker) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move { forward(Provider::Claude, input, &mut output).await });
    let response = b"{\"code\":\"synthetic-code#synthetic-state\"}\n";
    human.write_all(response).await.unwrap();
    let mut received = vec![0; response.len()];
    tokio::time::timeout(Duration::from_secs(1), worker.read_exact(&mut received))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received, response);
    assert!(
        !task.is_finished(),
        "a submitted code does not close the live session"
    );
    drop(human);
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        worker.read_u8().await.unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}

#[tokio::test]
async fn malformed_claude_input_and_all_codex_input_never_reach_the_worker() {
    for response in [
        b"{\"code\":\"one\\ntwo\"}\n".as_slice(),
        b"{\"code\":\"valid\",\"executable\":\"/bin/sh\"}\n",
        b"{\"code\":\"\"}\n",
        b"{\"code\":\"unterminated\"}",
    ] {
        let mut output = Vec::new();
        assert!(
            forward(Provider::Claude, response, &mut output)
                .await
                .is_err()
        );
        assert_eq!(output, [] as [u8; 0]);
    }
    let mut output = Vec::new();
    forward(
        Provider::Codex,
        b"{\"code\":\"valid\"}\n".as_slice(),
        &mut output,
    )
    .await
    .unwrap();
    assert_eq!(output, [] as [u8; 0]);
}
