use super::{Error, Result, run};
use crate::linux::http;
use std::{path::Path, time::Duration};
use zeroize::Zeroizing;

pub(super) const ORIGIN: &str = "unix:/var/lib/anima-daemon/link.sock";

pub(super) fn paired() -> bool {
    Path::new("/var/lib/anima-link/device.json").exists()
}

pub(super) async fn join(invite: &str) -> Result<()> {
    let input = Zeroizing::new(format!("{invite}\n"));
    run(
        "runuser",
        &[
            "-u",
            "anima-daemon",
            "--",
            "anima-link",
            "join",
            "-state",
            "/var/lib/anima-link",
        ],
        Some(input.as_bytes()),
        45,
    )
    .await?;
    Ok(())
}

pub(super) async fn start() -> Result<()> {
    run("systemctl", &["restart", "anima-link"], None, 45).await?;
    let client = http::client(true)?;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let result = client
                .get("http://localhost/health")
                .timeout(Duration::from_secs(2))
                .send()
                .await;
            if result.is_ok_and(|response| response.status().is_success()) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await
    .map_err(|_| {
        Error::Refused(
            "The private link could not reach its hub. Check the network, then try again.",
        )
    })
}
