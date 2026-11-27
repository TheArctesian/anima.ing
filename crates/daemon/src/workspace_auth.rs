//! Transient provider login worker inside a managed container. No credential-file access.
mod challenge;
mod claude;
use rustix::process::{Pid, Signal, kill_process_group};
use serde::Serialize;
use std::{
    io::{IsTerminal, Read},
    process::{ExitCode, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum Event<'a> {
    Challenge {
        verification_url: &'a str,
        code: &'a str,
    },
    PasteChallenge {
        verification_url: &'a str,
    },
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

/// Internal pipe mode; the authenticated execution owner controls its lifetime.
fn permitted() -> bool {
    !(std::io::stdout().is_terminal()
        || std::io::stdin().is_terminal()
        || std::env::var("ANIMA_MANAGED_OWNER_SOCKET").as_deref() != Ok("/run/anima/owner.sock")
        || std::env::var("HOME").as_deref() != Ok("/home/agent"))
}

pub async fn run() -> ExitCode {
    if !permitted() {
        return ExitCode::FAILURE;
    }
    finish(session().await).await
}

/// Fixed Claude login worker; the owner supplies a single bounded returned code.
pub async fn run_claude() -> ExitCode {
    if !permitted() {
        return ExitCode::FAILURE;
    }
    finish(claude::session().await).await
}

async fn finish(result: std::io::Result<()>) -> ExitCode {
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        let _ = emit(&Event::Failed).await;
        ExitCode::FAILURE
    }
}

async fn session() -> std::io::Result<()> {
    let environment =
        crate::threads::runner::login::environment(&std::env::vars().collect::<Vec<_>>()).await;
    let mut child = Command::new("/bin/codex")
        .args(["login", "--device-auth"])
        .current_dir("/home/agent")
        .env_clear()
        .envs(environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()?;
    let mut group = Group(
        child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .and_then(Pid::from_raw),
    );
    let mut output = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("missing provider stream"))?;
    let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
    // A native thread avoids leaving a Tokio blocking-stdin task alive at runtime shutdown.
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0_u8; 1]);
        let _ = cancel.send(());
    });
    let deadline = tokio::time::sleep(Duration::from_secs(600));
    tokio::pin!(deadline);
    let mut buffer = Zeroizing::new(Vec::new());
    let mut chunk = Zeroizing::new([0_u8; 2048]);
    let mut announced = false;
    let mut ended = false;
    let event = loop {
        tokio::select! {
            biased;
            _ = &mut cancelled => break Event::Cancelled,
            () = &mut deadline => break Event::Expired,
            read = output.read(chunk.as_mut()), if !ended => {
                let count = read?;
                if count == 0 { ended = true; continue; }
                if buffer.len() + count > 16384 { break Event::Failed; }
                buffer.extend_from_slice(&chunk[..count]);
                if !announced && let Some(code) = challenge::codex(&buffer) {
                    emit(&Event::Challenge { verification_url: challenge::VERIFICATION, code: &code }).await?;
                    announced = true;
                }
            }
            status = child.wait() => break if status?.success() && announced { Event::Succeeded } else { Event::Failed },
        }
    };
    group.kill();
    let _ = child.wait().await;
    emit(&event).await
}

async fn emit(event: &Event<'_>) -> std::io::Result<()> {
    let mut bytes = Zeroizing::new(serde_json::to_vec(event).map_err(std::io::Error::other)?);
    bytes.push(b'\n');
    let mut output = tokio::io::stdout();
    tokio::time::timeout(Duration::from_secs(5), async {
        output.write_all(&bytes).await?;
        output.flush().await
    })
    .await
    .map_err(|_| std::io::Error::other("login consumer disconnected"))?
}
struct Group(Option<Pid>);
impl Group {
    fn kill(&mut self) {
        if let Some(pid) = self.0.take() {
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        self.kill();
    }
}
