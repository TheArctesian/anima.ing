use crate::{Error, Result, boot, command, download, feed, state};
use serde_json::{Value, json};
use std::path::Path;

pub async fn check() -> Result<Value> {
    let release = feed::latest().await?;
    let previous = state::read()?;
    if release.version.as_str() < previous["highest"].as_str().unwrap_or("0") {
        return Err(Error::Refused(
            "The server offered an older release. It will not replace this system.",
        ));
    }
    let current = boot::current()?;
    Ok(
        json!({"available":release.system != current,"release":release,"current":current,"state":previous}),
    )
}

async fn download() -> Result<Value> {
    download_checked(check().await?).await
}

pub async fn download_checked(checked: Value) -> Result<Value> {
    let previous = state::read()?;
    if matches!(previous["phase"].as_str(), Some("staged" | "booting")) {
        return Err(Error::Refused(
            "Finish or roll back the pending update before downloading another.",
        ));
    }
    state::progress("checking", None, None)?;
    let result = stage(checked).await;
    let cleanup = match std::fs::remove_file(download::ARCHIVE) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Io(error)),
    };
    if result.is_ok() {
        state::clear_progress()?;
    }
    cleanup?;
    result
}

async fn stage(checked: Value) -> Result<Value> {
    let release: feed::Release = serde_json::from_value(checked["release"].clone())?;
    if checked["available"] == false {
        return Ok(json!({"phase":"current","version":release.version}));
    }
    download::space(&release)?;
    download::fetch(&release).await?;
    download::import(&release).await?;
    state::progress("staging", None, None)?;
    let current = boot::stage(&release.system).await?;
    let value = json!({"phase":"staged","version":release.version,"highest":release.version,"system":release.system,"previous":current});
    state::save(&value)?;
    Ok(value)
}

async fn health() -> Result<Value> {
    let mut pending = state::read()?;
    if pending["phase"] != "booting" {
        return Ok(pending);
    }
    let current = boot::current()?;
    if pending["system"] != current {
        boot::profile(&current).await?;
        pending["phase"] = json!("rolled-back");
        state::save(&pending)?;
        boot::select("anima-current.conf").await?;
        return Ok(pending);
    }
    let options = command::run("findmnt", &["-n", "-o", "OPTIONS", "/"]).await?;
    if !options.trim().split(',').any(|option| option == "ro") {
        return Err(Error::Refused("The system root is not protected."));
    }
    for unit in ["NetworkManager", "wpa_supplicant", "display-manager"] {
        command::run("systemctl", &["is-active", "--quiet", unit]).await?;
    }
    if Path::new("/var/lib/anima-daemon/daemon.json").exists() {
        command::run("systemctl", &["is-active", "--quiet", "anima-daemon"]).await?;
    }
    let bless = std::env::var("ANIMA_BLESS_BOOT")
        .unwrap_or_else(|_| "/run/current-system/sw/lib/systemd/systemd-bless-boot".into());
    command::run(&bless, &["good"]).await?;
    boot::profile(&current).await?;
    boot::pin("current", &current)?;
    pending["phase"] = json!("current");
    state::save(&pending)?;
    Ok(pending)
}

async fn rollback() -> Result<Value> {
    let mut pending = state::read()?;
    let previous = pending["previous"].as_str().ok_or(Error::Refused(
        "There is no previous update to roll back to.",
    ))?;
    if !Path::new(boot::ENTRIES)
        .join("anima-current.conf")
        .is_file()
    {
        return Err(Error::Refused(
            "There is no previous update to roll back to.",
        ));
    }
    boot::profile(previous).await?;
    boot::select("anima-current.conf").await?;
    pending["phase"] = json!("rolled-back");
    state::save(&pending)?;
    boot::reboot().await?;
    Ok(pending)
}

pub async fn dispatch(action: &str) -> Result<Value> {
    match action {
        "check" => check().await,
        "automatic" => crate::automatic::run().await,
        "download" => download().await,
        "health" => health().await,
        "rollback" => rollback().await,
        "activate" => {
            let mut pending = state::read()?;
            if pending["phase"] != "staged" {
                return Err(Error::Refused("Download and verify an update first."));
            }
            pending["phase"] = json!("booting");
            state::save(&pending)?;
            boot::activate().await?;
            Ok(pending)
        }
        "failure" => {
            let pending = state::read()?;
            if pending["phase"] == "booting" && pending["system"] == boot::current()? {
                boot::reboot().await?;
            }
            Ok(pending)
        }
        _ => Err(Error::Refused("Unknown update operation.")),
    }
}
