use crate::{Error, Result, command, state, update};
use serde_json::{Value, json};
use std::path::Path;

const RESULT: &str = "/persist/anima-updates/remote.json";
const ACTIONS: [&str; 4] = ["check", "download", "activate", "rollback"];

fn restarts(action: &str) -> bool {
    matches!(action, "activate" | "rollback")
}

fn unit(action: &str, instance: Option<&str>) -> String {
    let suffix = instance.map_or_else(String::new, |instance| format!("@{instance}"));
    format!("anima-update-remote-{action}{suffix}.service")
}

pub async fn dispatch(action: &str, review: Option<&str>) -> Result<Value> {
    if action == "status" {
        return status().await;
    }
    let (action, queued) = action
        .strip_suffix("-run")
        .map_or((action, false), |action| (action, true));
    if !ACTIONS.contains(&action) {
        return Err(Error::Refused("Unknown remote update action."));
    }
    let review = if restarts(action) {
        Some(
            review
                .filter(|revision| {
                    revision.len() == 64
                        && revision
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
                .ok_or(Error::Refused(
                    "Review the current update before requesting a remote restart.",
                ))?,
        )
    } else {
        None
    };
    if queued {
        return worker(action, review).await;
    }
    if let Some(review) = review {
        state::require_review(action, review)?;
    }
    command::run("systemctl", &["start", "--no-block", &unit(action, review)]).await?;
    Ok(json!({"requested":true}))
}

async fn worker(action: &str, review: Option<&str>) -> Result<Value> {
    let lease = crate::maintenance(action).await;
    let lock = state::lock()?;
    lock.try_lock()
        .map_err(|_| Error::Refused("Another update operation is running."))?;
    state::atomic(
        Path::new(RESULT),
        &serde_json::to_vec(&json!({"action":action,"running":true}))?,
    )?;
    let result = async {
        if let Some(review) = review {
            state::require_review(action, review)?;
        }
        update::dispatch(action, lease?).await
    }
    .await;
    let record = match &result {
        Ok(value) => json!({"action":action,"running":false,"value":value}),
        Err(error) => json!({"action":action,"running":false,"error":error.to_string()}),
    };
    state::atomic(Path::new(RESULT), &serde_json::to_vec(&record)?)?;
    result
}

async fn status() -> Result<Value> {
    let mut value = state::reviewed_status()?;
    let mut jobs = serde_json::Map::new();
    for action in ACTIONS {
        let review = value["review"][action].as_str();
        jobs.insert(action.into(), json!(job_state(action, review).await?));
    }
    let record = match state::read_bounded(Path::new(RESULT), 65536) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(error) => return Err(error),
    };
    value["remote"] = json!({"jobs":jobs,"result":record});
    Ok(value)
}

async fn job_state(action: &str, review: Option<&str>) -> Result<String> {
    if !restarts(action) {
        let text = command::run(
            "systemctl",
            &[
                "show",
                "--property=ActiveState",
                "--value",
                &unit(action, None),
            ],
        )
        .await?;
        return Ok(text.trim().to_owned());
    }
    let text = command::run(
        "systemctl",
        &[
            "list-units",
            "--all",
            "--plain",
            "--no-legend",
            "--no-pager",
            &unit(action, Some("*")),
        ],
    )
    .await?;
    let current = unit(action, review);
    let mut state = "inactive";
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let name = fields.next();
        match fields.nth(1) {
            Some(active @ ("active" | "activating")) => return Ok(active.to_owned()),
            Some("failed") if name == Some(current.as_str()) => state = "failed",
            _ => {}
        }
    }
    Ok(state.to_owned())
}
