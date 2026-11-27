//! Fixed provider login inside an owned workspace; challenges only cross live pipes.
use super::{Error, Result, workspace};
use agent_threads::managed::provider_login::{Login, Provider};
#[path = "provider_reply.rs"]
mod reply;
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    process::Command,
};
use uuid::Uuid;

pub(super) async fn run(
    login: Login,
    input: impl AsyncRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
) -> Result<()> {
    let Login {
        workspace,
        generation,
        principal,
        ..
    } = login;
    if !login.valid(chrono::Utc::now()) {
        return Err(Error::Refused("Provider sign-in authorization expired."));
    }
    let lifetime = (login.expires_at - chrono::Utc::now())
        .to_std()
        .map_err(|_| Error::Refused("Provider sign-in expired."))?;
    let deadline = tokio::time::Instant::now() + lifetime;
    check(workspace, generation, principal)?;
    let _lock = lock_workspace(workspace)?;
    let container = workspace::connection(workspace, generation).await?;
    check(workspace, generation, principal)?;
    if !login.valid(chrono::Utc::now()) {
        return Err(Error::Refused("Provider sign-in authorization expired."));
    }
    let mut child = launch(&container, login.provider)?;
    let mut control = child
        .stdin
        .take()
        .ok_or(Error::Refused("Provider control is unavailable."))?;
    let provider_output = child
        .stdout
        .take()
        .ok_or(Error::Refused("Provider output is unavailable."))?;
    tokio::time::timeout(Duration::from_secs(5), async {
        output
            .write_all(b"{\"ok\":true,\"stream\":\"workspace-provider/v1\"}\n")
            .await?;
        output.flush().await
    })
    .await
    .map_err(|_| Error::Refused("Provider sign-in consumer is unavailable."))??;
    let (outcome, completed) = tokio::select! {
        result = forward(provider_output, output) => (result, true),
        result = reply::forward(login.provider, input, &mut control) => (result, false),
        result = until_changed(workspace, generation, principal) => (result, false),
        () = tokio::time::sleep_until(deadline) => (Err(Error::Refused("Provider sign-in expired.")), false),
    };
    // EOF asks the in-container worker to end its provider process group.
    drop(control);
    let ended = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    if ended.is_err() {
        let _ = child.kill().await;
    }
    if completed && !matches!(ended, Ok(Ok(status)) if status.success()) {
        return Err(Error::Refused(
            "Provider sign-in did not complete successfully.",
        ));
    }
    outcome
}

fn lock_workspace(workspace: Uuid) -> Result<std::fs::File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(format!(
            "/run/anima-execution/provider-login-{workspace}.lock"
        ))?;
    lock.try_lock()
        .map_err(|_| Error::Refused("Provider sign-in is already open for this workspace."))?;
    Ok(lock)
}

fn launch(container: &str, provider: Provider) -> Result<tokio::process::Child> {
    Ok(Command::new("/run/current-system/sw/bin/docker")
        .args([
            "--config",
            "/etc/anima/docker-client",
            "--host",
            "unix:///run/docker.sock",
            "exec",
            "--interactive",
            "--user=1000:1000",
            container,
            "/bin/timeout",
            "--signal=TERM",
            "--kill-after=2s",
            "600s",
            "/bin/anima-daemon",
            match provider {
                Provider::Codex => "workspace-auth-codex",
                Provider::Claude => "workspace-auth-claude",
            },
        ])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?)
}

fn check(id: Uuid, generation: u64, principal: Uuid) -> Result<()> {
    if id.is_nil() || principal.is_nil() || generation == 0 {
        return Err(Error::Refused("Invalid provider sign-in identity."));
    }
    let record = workspace::record::required(id)?;
    if record.principal != Some(principal)
        || record.generation != generation
        || record.phase != workspace::record::Phase::Running
        || !record.checkout_ready()
    {
        return Err(Error::Refused(
            "Provider sign-in no longer owns this running workspace.",
        ));
    }
    Ok(())
}

async fn until_changed(id: Uuid, generation: u64, principal: Uuid) -> Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        check(id, generation, principal)?;
    }
}

async fn forward(input: impl AsyncRead + Unpin, mut output: impl AsyncWrite + Unpin) -> Result<()> {
    let mut bounded = input.take(4097);
    let copied = tokio::io::copy(&mut bounded, &mut output).await?;
    if copied > 4096 {
        return Err(Error::Refused(
            "Provider sign-in output exceeded its limit.",
        ));
    }
    Ok(())
}
