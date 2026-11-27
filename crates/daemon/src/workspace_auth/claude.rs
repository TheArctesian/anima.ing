//! Fixed Claude CLI login subprocess with one human reply and a bounded lifetime.
use super::{Event, Group, emit};
#[path = "claude_challenge.rs"]
mod protocol;
use rustix::process::Pid;
use std::{io, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
};
use zeroize::Zeroizing;

async fn spawn() -> io::Result<Child> {
    let environment =
        crate::threads::runner::login::environment(&std::env::vars().collect::<Vec<_>>()).await;
    Command::new("/bin/claude")
        .args(["auth", "login", "--claudeai"])
        .current_dir("/home/agent")
        .env_clear()
        .envs(environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
}

pub(super) async fn session() -> io::Result<()> {
    let mut child = spawn().await?;
    let mut group = Group(
        child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .and_then(Pid::from_raw),
    );
    let mut output = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing provider stream"))?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing provider input"))?;
    let mut replies = protocol::replies();
    let deadline = tokio::time::sleep(Duration::from_secs(600));
    tokio::pin!(deadline);
    let mut buffer = Zeroizing::new(Vec::new());
    let mut chunk = Zeroizing::new([0_u8; 2048]);
    let mut announced = false;
    let mut submitted = false;
    let mut ended = false;
    let event = loop {
        tokio::select! {
            biased;
            () = &mut deadline => break Event::Expired,
            reply = replies.recv() => {
                if let Some(event) = submit(reply, &mut input, announced && !submitted).await? {
                    break event;
                }
                submitted = true;
            }
            read = output.read(chunk.as_mut()), if !ended => {
                ended = consume(&chunk[..read?], &mut buffer, &mut announced).await?;
            }
            status = child.wait() => break if status?.success() && announced && submitted { Event::Succeeded } else { Event::Failed },
        }
    };
    group.kill();
    let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    emit(&event).await
}

async fn consume(
    bytes: &[u8],
    buffer: &mut Zeroizing<Vec<u8>>,
    announced: &mut bool,
) -> io::Result<bool> {
    if buffer.len() + bytes.len() > 16384 {
        return Err(io::Error::other("provider output exceeded limit"));
    }
    buffer.extend_from_slice(bytes);
    if !*announced && let Some(url) = protocol::challenge(buffer) {
        emit(&Event::PasteChallenge {
            verification_url: &url,
        })
        .await?;
        *announced = true;
    }
    Ok(bytes.is_empty())
}

async fn submit(
    reply: Option<Zeroizing<String>>,
    input: &mut ChildStdin,
    permitted: bool,
) -> io::Result<Option<Event<'static>>> {
    let Some(mut code) = reply else {
        return Ok(Some(Event::Cancelled));
    };
    if !permitted {
        return Ok(Some(Event::Failed));
    }
    code.push('\n');
    tokio::time::timeout(Duration::from_secs(5), input.write_all(code.as_bytes()))
        .await
        .map_err(|_| io::Error::other("provider input stalled"))??;
    Ok(None)
}
