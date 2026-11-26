use super::{Error, Result, previews, workspace};
use std::process::Stdio;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    process::Command,
};
use uuid::Uuid;

/// Streams the existing runner protocol only after the local owner boundary has
/// resolved the exact workspace generation. No host socket path comes from input.
pub async fn connect(id: Uuid, generation: u64, input: impl AsyncRead + Unpin) -> Result<()> {
    stream(
        Endpoint {
            workspace: id,
            generation,
            destination: Destination::Runner,
        },
        input,
        tokio::io::stdout(),
    )
    .await
}

/// The caller must hold the workspace preview grant. This local root-only
/// transport never publishes a host port or accepts an arbitrary address.
pub async fn preview(lease: previews::Lease, input: impl AsyncRead + Unpin) -> Result<()> {
    preview_to(lease, input, tokio::io::stdout()).await
}

pub async fn preview_to(
    lease: previews::Lease,
    input: impl AsyncRead + Unpin,
    output: impl AsyncWrite + Unpin,
) -> Result<()> {
    let binding = &lease.binding;
    tokio::select! {
        result = stream(Endpoint { workspace: binding.workspace, generation: binding.generation, destination: Destination::Preview(binding.port) }, input, output) => result,
        result = previews::until_closed(&lease) => result,
    }
}

#[derive(Clone, Copy)]
enum Destination {
    Runner,
    Preview(u16),
}

struct Endpoint {
    workspace: Uuid,
    generation: u64,
    destination: Destination,
}

async fn stream(
    endpoint: Endpoint,
    mut input: impl AsyncRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
) -> Result<()> {
    let container = workspace::connection(endpoint.workspace, endpoint.generation).await?;
    let (mut child, protocol) = connect_container(&container, endpoint.destination)?;
    let mut sending = child
        .stdin
        .take()
        .ok_or(Error::Refused("Runner input is unavailable."))?;
    let mut reading = child
        .stdout
        .take()
        .ok_or(Error::Refused("Runner output is unavailable."))?;
    let mut handshake = serde_json::to_vec(&serde_json::json!({"ok":true,"stream":protocol}))?;
    handshake.push(b'\n');
    output.write_all(&handshake).await?;
    output.flush().await?;
    let send = async {
        tokio::io::copy(&mut input, &mut sending).await?;
        sending.shutdown().await?;
        drop(sending);
        Ok::<(), std::io::Error>(())
    };
    let receive = async { tokio::io::copy(&mut reading, &mut output).await.map(drop) };
    tokio::try_join!(send, receive)?;
    if !child.wait().await?.success() {
        return Err(Error::Refused(
            "The workspace connection ended. Reconnect to the same generation.",
        ));
    }
    Ok(())
}

fn connect_container(
    container: &str,
    destination: Destination,
) -> Result<(tokio::process::Child, &'static str)> {
    let (user, address, protocol) = match destination {
        Destination::Runner => (
            "--user=0:0",
            "UNIX-CONNECT:/run/anima/owner.sock".to_owned(),
            "agent-threads/v1",
        ),
        Destination::Preview(port) => (
            "--user=1000:1000",
            format!("TCP4:127.0.0.1:{port},connect-timeout=10"),
            "workspace-preview/v1",
        ),
    };
    let child = Command::new("/run/current-system/sw/bin/docker")
        .args([
            "--config",
            "/etc/anima/docker-client",
            "--host",
            "unix:///run/docker.sock",
            "exec",
            "--interactive",
            user,
            container,
            "/bin/timeout",
            "--signal=TERM",
            "--kill-after=2s",
            "600s",
            "/bin/socat",
            "STDIO",
            &address,
        ])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    Ok((child, protocol))
}
