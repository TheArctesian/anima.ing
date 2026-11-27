use crate::{Error, RECEIPT, Receipt, lock};
use serde_json::Value;
use std::{fs, io::Read, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
use uuid::Uuid;

/// # Errors
/// Refuses malformed or oversized maintenance state.
pub fn status() -> Result<Value, Error> {
    let file = match fs::File::open(RECEIPT) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused("Maintenance receipt exceeded its limit."));
    }
    let receipt: Receipt = serde_json::from_slice(&bytes)?;
    Ok(serde_json::to_value(receipt)?)
}

/// Explicit owner recovery only; never called after a timeout automatically.
/// # Errors
/// Refuses stale identities, pending power jobs and unknown system state.
pub async fn release(id: Uuid) -> Result<(), Error> {
    let _lock = lock()?;
    let receipt: Receipt = serde_json::from_value(status()?)?;
    if receipt.schema != 1 || receipt.id != id {
        return Err(Error::Refused(
            "Maintenance state changed. Refresh before reconciling.",
        ));
    }
    let mut child = Command::new("/run/current-system/sw/bin/systemctl")
        .args(["list-jobs", "--no-legend", "--no-pager"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("Power job state is unavailable."))?;
    let observe = async {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).await?;
        if bytes.len() > 65536 || !child.wait().await?.success() {
            return Err(Error::Refused("Power job state is unknown."));
        }
        let jobs = std::str::from_utf8(&bytes)
            .map_err(|_| Error::Refused("Power job state is invalid."))?;
        if jobs.lines().any(|line| {
            [
                "reboot",
                "poweroff",
                "shutdown",
                "halt",
                "kexec",
                "soft-reboot",
            ]
            .iter()
            .any(|name| line.contains(name))
        }) {
            return Err(Error::Refused(
                "A power transition is still pending. Admission remains closed.",
            ));
        }
        Ok(())
    };
    tokio::time::timeout(Duration::from_secs(10), observe)
        .await
        .map_err(|_| Error::Refused("Power job observation timed out."))??;
    fs::remove_file(RECEIPT)?;
    Ok(())
}
