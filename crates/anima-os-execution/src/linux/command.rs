use super::{Error, Result};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub async fn run(program: &str, args: &[&str]) -> Result<String> {
    capture(program, args, None, 60).await
}

async fn capture(
    program: &str,
    args: &[&str],
    input: Option<&[u8]>,
    seconds: u64,
) -> Result<String> {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/run/current-system/sw/bin")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("Runtime output is unavailable."))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or(Error::Refused("Runtime input is unavailable."))?;
    let read = async {
        let send = async {
            if let Some(input) = input {
                stdin.write_all(input).await?;
            }
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<(), std::io::Error>(())
        };
        let receive = async {
            let mut bytes = Vec::new();
            stdout.take(1_048_577).read_to_end(&mut bytes).await?;
            Ok::<_, std::io::Error>(bytes)
        };
        let ((), bytes) = tokio::try_join!(send, receive)?;
        if bytes.len() > 1_048_576 || !child.wait().await?.success() {
            return Err(Error::Refused(
                "The workspace runtime operation failed. Reconcile its state before retrying.",
            ));
        }
        String::from_utf8(bytes).map_err(|_| Error::Refused("Runtime output is invalid."))
    };
    tokio::time::timeout(Duration::from_secs(seconds), read)
        .await
        .map_err(|_| Error::Refused("The workspace runtime timed out; its outcome is unknown."))?
}

pub async fn docker(args: &[&str]) -> Result<String> {
    let mut fixed = vec![
        "--config",
        "/etc/anima/docker-client",
        "--host",
        "unix:///run/docker.sock",
    ];
    fixed.extend_from_slice(args);
    run("/run/current-system/sw/bin/docker", &fixed).await
}

pub async fn docker_input(args: &[&str], input: &[u8]) -> Result<String> {
    docker_input_for(args, input, 10).await
}

pub async fn docker_input_for(args: &[&str], input: &[u8], seconds: u64) -> Result<String> {
    let mut fixed = vec![
        "--config",
        "/etc/anima/docker-client",
        "--host",
        "unix:///run/docker.sock",
    ];
    fixed.extend_from_slice(args);
    capture(
        "/run/current-system/sw/bin/docker",
        &fixed,
        Some(input),
        seconds,
    )
    .await
}
