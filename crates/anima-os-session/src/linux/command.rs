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
    let (success, text, _) = capture(program, args, input, seconds).await?;
    if !success {
        return Err(Error::Refused(
            "The operation failed. Check the connection and try again.",
        ));
    }
    Ok(text)
}

pub async fn unit_state(name: &str) -> String {
    match run(
        "systemctl",
        &["show", "--property=ActiveState", "--value", "--", name],
        None,
        3,
    )
    .await
    {
        Ok(value)
            if ["active", "inactive", "failed", "activating", "deactivating"]
                .contains(&value.as_str()) =>
        {
            value
        }
        _ => "unknown".into(),
    }
}

pub async fn json_reply(program: &str, args: &[&str], seconds: u64) -> Result<serde_json::Value> {
    let (_, text, _) = capture(program, args, None, seconds).await?;
    Ok(serde_json::from_str(&text)?)
}

pub(super) async fn partial_stdout(program: &str, args: &[&str], seconds: u64) -> Result<String> {
    let (_, text, _) = capture(program, args, None, seconds).await?;
    Ok(text)
}

async fn capture(
    program: &str,
    args: &[&str],
    input: Option<&[u8]>,
    seconds: u64,
) -> Result<(bool, String, String)> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
    let stderr = child
        .stderr
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
        let ((), bytes, errors) = tokio::try_join!(write, bounded(stdout), bounded(stderr))?;
        let success = child.wait().await?.success();
        let text = String::from_utf8(bytes)
            .map_err(|_| Error::Refused("Operation returned invalid text."))?;
        let errors = String::from_utf8(errors)
            .map_err(|_| Error::Refused("Operation returned invalid text."))?;
        Ok((success, text.trim().to_owned(), errors.trim().to_owned()))
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

async fn bounded(reader: impl tokio::io::AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(1_048_577).read_to_end(&mut bytes).await?;
    if bytes.len() > 1_048_576 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "Operation output exceeded its limit.",
        ));
    }
    Ok(bytes)
}

pub async fn streams(program: &str, args: &[&str], seconds: u64) -> Result<(String, String)> {
    let (success, output, errors) = capture(program, args, None, seconds).await.map_err(|error| {
        if matches!(&error, Error::Io(cause) if cause.kind() == std::io::ErrorKind::FileTooLarge) {
            Error::Refused("This log selection is too large. Choose fewer lines.")
        } else {
            error
        }
    })?;
    if !success {
        return Err(Error::Refused(
            "Container logs are unavailable. Refresh the inventory and retry.",
        ));
    }
    Ok((output, errors))
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
