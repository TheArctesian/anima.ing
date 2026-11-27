use super::{
    Error, Result,
    command::{atomic, lock},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use uuid::Uuid;

const PREVIEW: &str = "/run/anima-display-preview.json";
mod cli;
mod output;
mod physical;
pub(super) use cli::{cli, print_list};
pub(super) use output::list;
use output::{ROTATIONS, outputs, rotate};
use physical::run;
#[derive(Serialize, Deserialize)]
struct Preview {
    id: Uuid,
    output: String,
    previous: String,
    rotation: String,
    expires: f64,
}

fn uptime() -> Result<f64> {
    std::fs::read_to_string("/proc/uptime")?
        .split_whitespace()
        .next()
        .and_then(|word| word.parse().ok())
        .ok_or(Error::Refused("Display deadline unavailable."))
}

fn read() -> Result<Option<Preview>> {
    if !Path::new(PREVIEW).exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(PREVIEW)?;
    if bytes.len() > 4096 {
        return Err(Error::Refused("Display recovery record is invalid."));
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

async fn restore(preview: &Preview) -> Result<()> {
    rotate(&preview.output, &preview.previous).await?;
    std::fs::remove_file(PREVIEW)?;
    Ok(())
}

pub async fn revert(token: Option<Uuid>) -> Result<()> {
    let _lease = recovery_lock().await?;
    if let Some(preview) = read()?
        && token.is_none_or(|token| token == preview.id)
    {
        restore(&preview).await?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choice {
    output: String,
    rotation: String,
}

pub async fn preview(args: Value) -> Result<Uuid> {
    let choice: Choice = serde_json::from_value(args)?;
    let _lease = lock("/run/anima-display.lock")?;
    if let Some(previous) = read()? {
        restore(&previous).await?;
    }
    let current = outputs(&run("xrandr", &["--prop"], None, 5).await?)
        .into_iter()
        .find(|output| output.name == choice.output)
        .ok_or(Error::Refused("Choose a connected display."))?;
    if !ROTATIONS.contains(&choice.rotation.as_str()) {
        return Err(Error::Refused("Choose a supported orientation."));
    }
    let preview = Preview {
        id: Uuid::now_v7(),
        output: choice.output,
        previous: current.rotation,
        rotation: choice.rotation,
        expires: uptime()? + 15.0,
    };
    atomic(Path::new(PREVIEW), &serde_json::to_vec(&preview)?, 0o600)?;
    let timer = schedule(&preview).await;
    if timer.is_err() {
        std::fs::remove_file(PREVIEW)?;
        timer?;
    }
    if let Err(error) = rotate(&preview.output, &preview.rotation).await {
        restore(&preview).await?;
        return Err(error);
    }
    Ok(preview.id)
}

pub async fn keep(token: Uuid) -> Result<Value> {
    let _lease = lock("/run/anima-display.lock")?;
    let preview = read()?.ok_or(Error::Refused(
        "The display preview expired. Apply it again.",
    ))?;
    if preview.id != token {
        return Err(Error::Refused(
            "The display preview belongs to another session.",
        ));
    }
    if uptime()? >= preview.expires {
        restore(&preview).await?;
        return Err(Error::Refused(
            "The display preview expired. Apply it again.",
        ));
    }
    let current = outputs(&run("xrandr", &["--prop"], None, 5).await?)
        .into_iter()
        .find(|output| output.name == preview.output);
    if current.is_none_or(|output| output.rotation != preview.rotation) {
        restore(&preview).await?;
        return Err(Error::Refused(
            "Display changed. Apply the orientation again.",
        ));
    }
    if uptime()? >= preview.expires {
        restore(&preview).await?;
        return Err(Error::Refused("The preview expired. Apply it again."));
    }
    let persistent = run(
        "findmnt",
        &[
            "--mountpoint",
            "/persist",
            "--noheadings",
            "--output",
            "TARGET",
        ],
        None,
        3,
    )
    .await
    .is_ok();
    let path = Path::new(if persistent {
        "/persist/display.json"
    } else {
        "/run/anima-display.json"
    });
    let mut saved: serde_json::Map<String, Value> = if path.exists() {
        serde_json::from_slice(&std::fs::read(path)?)?
    } else {
        serde_json::Map::new()
    };
    saved.insert(preview.output, Value::String(preview.rotation));
    atomic(path, &serde_json::to_vec(&saved)?, 0o644)?;
    std::fs::remove_file(PREVIEW)?;
    Ok(json!({}))
}

pub async fn auto() -> Result<()> {
    if nix::unistd::Uid::effective().is_root() {
        revert(None).await?;
    }
    let saved: serde_json::Map<String, Value> = std::fs::read("/persist/display.json")
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    for output in outputs(&run("xrandr", &["--prop"], None, 5).await?) {
        let configured = saved
            .get(&output.name)
            .and_then(Value::as_str)
            .filter(|rotation| ROTATIONS.contains(rotation));
        let panel = if ["eDP", "DSI", "LVDS"]
            .iter()
            .any(|prefix| output.name.starts_with(prefix))
        {
            match output.panel.as_deref() {
                Some("Normal") => Some("normal"),
                Some("Upside Down") => Some("inverted"),
                Some("Left Side Up") => Some("left"),
                Some("Right Side Up") => Some("right"),
                _ => None,
            }
        } else {
            None
        };
        if let Some(rotation) = configured.or(panel) {
            rotate(&output.name, rotation).await?;
        }
    }
    Ok(())
}

async fn schedule(preview: &Preview) -> Result<String> {
    let executable = std::env::current_exe()?;
    let id = preview.id.to_string();
    let unit = format!("--unit=anima-display-{id}");
    let display = format!(
        "--setenv=DISPLAY={}",
        std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into())
    );
    let path = format!(
        "--setenv=PATH={}",
        std::env::var("PATH").unwrap_or_default()
    );
    let authority = format!(
        "--setenv=XAUTHORITY={}",
        std::env::var("XAUTHORITY").unwrap_or_default()
    );
    run(
        "systemd-run",
        &[
            &unit,
            "--collect",
            "--on-active=15s",
            "--timer-property=AccuracySec=1s",
            &display,
            &authority,
            &path,
            executable
                .to_str()
                .ok_or(Error::Refused("Display guard unavailable."))?,
            "display-revert",
            &id,
        ],
        None,
        5,
    )
    .await
}

async fn recovery_lock() -> Result<std::fs::File> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(6);
    loop {
        match lock("/run/anima-display.lock") {
            Ok(lease) => return Ok(lease),
            Err(error) if tokio::time::Instant::now() >= deadline => return Err(error),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
}
