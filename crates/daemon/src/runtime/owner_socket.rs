use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub(crate) async fn call(
    socket: &str,
    request: &impl Serialize,
    bounds: (usize, usize),
) -> Option<Value> {
    call_input(socket, request, bounds, None).await
}

pub(crate) async fn call_input(
    socket: &str,
    request: &impl Serialize,
    bounds: (usize, usize),
    input: Option<&[u8]>,
) -> Option<Value> {
    tokio::time::timeout(
        Duration::from_secs(70),
        exchange(socket, request, bounds, input),
    )
    .await
    .ok()
    .flatten()
}
async fn exchange(
    socket: &str,
    request: &impl Serialize,
    (input_limit, output_limit): (usize, usize),
    input: Option<&[u8]>,
) -> Option<Value> {
    let stream = UnixStream::connect(socket).await.ok()?;
    if stream.peer_cred().ok()?.uid() != 0 {
        return None;
    }
    let (mut reading, mut writing) = stream.into_split();
    let mut bytes = serde_json::to_vec(request).ok()?;
    if bytes.len() >= input_limit {
        return None;
    }
    bytes.push(b'\n');
    writing.write_all(&bytes).await.ok()?;
    if let Some(input) = input {
        let mut ready = [0; 6];
        reading.read_exact(&mut ready).await.ok()?;
        if &ready != daemon_core::source_jobs::CHECKOUT_READY {
            return None;
        }
        writing.write_all(input).await.ok()?;
        writing.shutdown().await.ok()?;
    }
    let mut reply = Vec::new();
    BufReader::new(reading)
        .take(output_limit as u64 + 2)
        .read_until(b'\n', &mut reply)
        .await
        .ok()?;
    if reply.len() > output_limit + 1 || reply.last() != Some(&b'\n') {
        return None;
    }
    let reply: Value = serde_json::from_slice(&reply).ok()?;
    (reply["ok"] == true).then(|| reply["value"].clone())
}
