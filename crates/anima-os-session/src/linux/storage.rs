use super::{Error, Result, command};
use serde_json::Value;

pub async fn data_volumes(args: Value) -> Result<Value> {
    if !args.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(Error::Refused("Invalid data volume request."));
    }
    let snapshot = command::run(
        "/run/current-system/sw/bin/anima-os-storage",
        &["inventory"],
        None,
        10,
    )
    .await
    .map_err(|_| Error::Refused("Data volume status is unavailable."))?;
    Ok(serde_json::from_str(&snapshot)?)
}
