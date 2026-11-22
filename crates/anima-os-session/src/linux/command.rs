use super::{Error, Result};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub async fn run(
    program: &str,
    args: &[&str],
    input: Option<&[u8]>,
    seconds: u64,
) -> Result<String> {
    let (success, text) = capture(program, args, input, seconds).await?;
    if !success {
        return Err(Error::Refused(
            "The operation failed. Check the connection and try again.",
        ));
    }
    Ok(text)
}

pub async fn json_reply(program: &str, args: &[&str], seconds: u64) -> Result<serde_json::Value> {
    let (_, text) = capture(program, args, None, seconds).await?;
    Ok(serde_json::from_str(&text)?)
}

async fn capture(
    program: &str,
    args: &[&str],
    input: Option<&[u8]>,
    seconds: u64,
) -> Result<(bool, String)> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or(Error::Refused("Operation input unavailable."))?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("Operation output unavailable."))?;
    let request = async {
        let write = async {
            if let Some(input) = input {
                stdin.write_all(input).await?;
            }
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let read = async {
            let mut bytes = Vec::new();
            stdout.take(1_048_577).read_to_end(&mut bytes).await?;
            Ok::<_, std::io::Error>(bytes)
        };
        let ((), bytes) = tokio::try_join!(write, read)?;
        if bytes.len() > 1_048_576 {
            return Err(Error::Refused("Operation output exceeded its limit."));
        }
        let success = child.wait().await?.success();
        String::from_utf8(bytes)
            .map(|text| (success, text.trim().to_owned()))
            .map_err(|_| Error::Refused("Operation returned invalid text."))
    };
    let result = tokio::time::timeout(Duration::from_secs(seconds), request)
        .await
        .unwrap_or(Err(Error::Refused(
            "The operation timed out. Check its state before retrying.",
        )));
    if result.is_err() && child.try_wait()?.is_none() {
        child.kill().await?;
    }
    result
}

pub fn lock(path: &str) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    file.try_lock().map_err(|_| {
        Error::Refused("Another operation is already running. Wait for it to finish.")
    })?;
    Ok(file)
}

pub fn atomic(path: &std::path::Path, bytes: &[u8], mode: u32) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let temporary = path.with_extension("pending");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)?;
    std::fs::File::open(path.parent().ok_or(Error::Refused("Invalid state path."))?)?.sync_all()?;
    Ok(())
}
