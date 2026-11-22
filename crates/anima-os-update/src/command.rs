use crate::{Error, Result};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

pub async fn run(program: &str, arguments: &[&str]) -> Result<String> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("The update command could not be observed."))?;
    let collect = async {
        let mut bytes = Vec::new();
        stdout.take(1_048_577).read_to_end(&mut bytes).await?;
        if bytes.len() > 1_048_576 {
            return Err(Error::Refused(
                "The update command returned too much output.",
            ));
        }
        if !child.wait().await?.success() {
            return Err(Error::Refused(
                "The update system command failed. Check diagnostics before retrying.",
            ));
        }
        String::from_utf8(bytes)
            .map_err(|_| Error::Refused("The update command returned invalid text."))
    };
    let result = timeout(Duration::from_secs(120), collect)
        .await
        .unwrap_or(Err(Error::Refused("The update system command timed out.")));
    if result.is_err() && child.try_wait()?.is_none() {
        child.kill().await?;
    }
    result
}
