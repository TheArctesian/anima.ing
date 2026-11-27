//! One bounded request to the existing runner; no shell or caller-selected socket.
use super::super::{Error, Result};
use agent_threads::control::{Call, Outcome, Reply};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

pub(super) async fn call(container: &str, call: Call) -> Result<Outcome> {
    exchange(container, call, 60).await
}

pub(super) async fn probe(container: &str, call: Call) -> Result<Outcome> {
    exchange(container, call, 2).await
}

async fn exchange(container: &str, call: Call, seconds: u64) -> Result<Outcome> {
    let guest_limit = format!("{}s", seconds + 5);
    let mut child = runner_transport(container, &guest_limit)?;
    let mut input = child
        .stdin
        .take()
        .ok_or(Error::Refused("Runner input is unavailable."))?;
    let output = child
        .stdout
        .take()
        .ok_or(Error::Refused("Runner output is unavailable."))?;
    let mut frame = serde_json::to_vec(&call)?;
    frame.push(b'\n');
    let exchange = async {
        input.write_all(&frame).await?;
        input.flush().await?;
        let mut answer = Vec::new();
        BufReader::new(output)
            .take(1_048_577)
            .read_until(b'\n', &mut answer)
            .await?;
        if answer.len() > 1_048_576 || answer.last() != Some(&b'\n') {
            return Err(Error::Refused(
                "The runner response is incomplete or too large.",
            ));
        }
        match serde_json::from_slice::<Reply>(&answer)? {
            Reply::Done { id, result } if id == call.id => Ok(result),
            _ => Err(Error::Refused(
                "The runner response did not match this command.",
            )),
        }
    };
    let answer = tokio::time::timeout(Duration::from_secs(seconds), exchange)
        .await
        .map_err(|_| {
            Error::Refused(
                "The command outcome is unresolved. Do not repeat it with a new identity.",
            )
        })?;
    // Closing the transport does not stop the thread managed by the holder.
    drop(input);
    let _ = child.kill().await;
    answer
}

fn runner_transport(container: &str, guest_limit: &str) -> Result<tokio::process::Child> {
    Ok(Command::new("/run/current-system/sw/bin/docker")
        .args([
            "--config",
            "/etc/anima/docker-client",
            "--host",
            "unix:///run/docker.sock",
            "exec",
            "--interactive",
            "--user=0:0",
            container,
            "/bin/timeout",
            "--signal=TERM",
            "--kill-after=2s",
            guest_limit,
            "/bin/socat",
            "STDIO",
            "UNIX-CONNECT:/run/anima/owner.sock",
        ])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?)
}
