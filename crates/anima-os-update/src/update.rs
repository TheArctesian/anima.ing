use crate::{Error, Result, boot, command, download, feed, state};
use anima_os_maintenance::Admission;
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
    let _admission = Admission::configuration()?;
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
    let _admission = Admission::configuration()?;
    let current = boot::current()?;
    if pending["system"] != current {
        boot::check_rollback(&current)?;
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
    boot::check_current(&current)?;
    let bless = std::env::var("ANIMA_BLESS_BOOT")
        .unwrap_or_else(|_| "/run/current-system/sw/lib/systemd/systemd-bless-boot".into());
    command::run(&bless, &["good"]).await?;
    boot::profile(&current).await?;
    boot::pin("current", &current)?;
    pending["phase"] = json!("current");
    state::save(&pending)?;
    Ok(pending)
}

async fn rollback(mut lease: anima_os_maintenance::Lease) -> Result<Value> {
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
    boot::check_rollback(previous)?;
    lease.dispatching()?;
    boot::profile(previous).await?;
    boot::select("anima-current.conf").await?;
    pending["phase"] = json!("rolled-back");
    state::save(&pending)?;
    boot::reboot().await?;
    Ok(pending)
}

pub fn retry_needed() -> Result<bool> {
    candidate_started(&state::read()?)
}

fn candidate_started(pending: &Value) -> Result<bool> {
    Ok(pending["phase"] == "booting" && pending["system"] == boot::current()?)
}

pub async fn dispatch(action: &str, lease: Option<anima_os_maintenance::Lease>) -> Result<Value> {
    match action {
        "check" => check().await,
        "automatic" => crate::automatic::run().await,
        "download" => download().await,
        "health" => health().await,
        "rollback" => {
            rollback(lease.ok_or(Error::Refused("Maintenance ownership is required."))?).await
        }
        "activate" => {
            let lease = lease.ok_or(Error::Refused("Maintenance ownership is required."))?;
            activate(activation_target()?, lease).await
        }
        "failure" => {
            let pending = state::read()?;
            if candidate_started(&pending)? {
                let mut lease =
                    lease.ok_or(Error::Refused("Maintenance ownership is required."))?;
                boot::check_boot_retry(system_of(&pending, "previous")?)?;
                lease.dispatching()?;
                boot::reboot().await?;
            }
            Ok(pending)
        }
        _ => Err(Error::Refused("Unknown update operation.")),
    }
}

pub fn activation_target() -> Result<Value> {
    let pending = state::read()?;
    if pending["phase"] != "staged" {
        return Err(Error::Refused("Download and verify an update first."));
    }
    boot::check_activation(
        system_of(&pending, "system")?,
        system_of(&pending, "previous")?,
    )?;
    Ok(pending)
}

pub async fn activate(mut pending: Value, mut lease: anima_os_maintenance::Lease) -> Result<Value> {
    lease.dispatching()?;
    pending["phase"] = json!("booting");
    state::save(&pending)?;
    boot::activate().await?;
    Ok(pending)
}

pub(super) fn system_of<'a>(pending: &'a Value, key: &str) -> Result<&'a str> {
    pending[key]
        .as_str()
        .ok_or(Error::Refused("The retained update systems are invalid."))
}
