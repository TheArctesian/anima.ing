use crate::{Error, Result, command, state, update};
use serde_json::{Value, json};
use std::path::Path;

const RESULT: &str = "/persist/anima-updates/remote.json";

fn unit(action: &str) -> Result<&'static str> {
    match action {
        "check" => Ok("anima-update-remote-check.service"),
        "download" => Ok("anima-update-remote-download.service"),
        "activate" => Ok("anima-update-remote-activate.service"),
        "rollback" => Ok("anima-update-remote-rollback.service"),
        _ => Err(Error::Refused("Unknown remote update action.")),
    }
}

pub async fn dispatch(action: &str) -> Result<Value> {
    if action == "status" {
        return status().await;
    }
    if let Some(action) = action.strip_suffix("-run") {
        unit(action)?;
        return worker(action).await;
    }
    let unit = unit(action)?;
    command::run("systemctl", &["start", "--no-block", unit]).await?;
    Ok(json!({"requested":true}))
}

async fn worker(action: &str) -> Result<Value> {
    let lock = state::lock()?;
    lock.try_lock()
        .map_err(|_| Error::Refused("Another update operation is running."))?;
    state::atomic(
        Path::new(RESULT),
        &serde_json::to_vec(&json!({"action":action,"running":true}))?,
    )?;
    let result = update::dispatch(action).await;
    let record = match &result {
        Ok(value) => json!({"action":action,"running":false,"value":value}),
        Err(error) => json!({"action":action,"running":false,"error":error.to_string()}),
    };
    state::atomic(Path::new(RESULT), &serde_json::to_vec(&record)?)?;
    result
}

async fn status() -> Result<Value> {
    let mut value = state::status()?;
    let mut jobs = serde_json::Map::new();
    for action in ["check", "download", "activate", "rollback"] {
        let text = command::run(
            "systemctl",
            &["show", "--property=ActiveState", "--value", unit(action)?],
        )
        .await?;
        jobs.insert(action.into(), json!(text.trim()));
    }
    let record = match state::read_bounded(Path::new(RESULT), 65536) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(error) => return Err(error),
    };
    value["remote"] = json!({"jobs":jobs,"result":record});
    Ok(value)
}
