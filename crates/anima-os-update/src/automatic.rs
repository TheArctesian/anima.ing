use crate::{Result, feed, state, update};
use serde_json::{Value, json};

/// Timer runs never activate a system or retry the release that last rolled back.
pub async fn run() -> Result<Value> {
    let retained = state::read()?;
    if matches!(retained["phase"].as_str(), Some("staged" | "booting")) {
        return Ok(retained);
    }
    let checked = update::check().await?;
    let release: feed::Release = serde_json::from_value(checked["release"].clone())?;
    if checked["available"] == false {
        return Ok(json!({"phase":"current","version":release.version}));
    }
    if retained["phase"] == "rolled-back" && retained["version"] == release.version {
        return Ok(json!({"phase":"held","reason":"previous-rollback","version":release.version}));
    }
    update::download_checked(checked).await
}
