//! Read only the selected build attempt's owner-filtered diagnostic snapshot.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{EXECUTION, Error, Result, command};

const UNAVAILABLE: &str = "No saved diagnostics are available for this build.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    attempt: Uuid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Program {
    Railpack,
    Buildctl,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    attempt: Uuid,
    program: Program,
    stage: String,
    finished: bool,
    stdout: String,
    stderr: String,
}

pub(super) async fn read(args: Value) -> Result<Value> {
    let Request { attempt } = serde_json::from_value(args)?;
    if attempt.is_nil() {
        return Err(Error::Refused("Choose a build from the current inventory."));
    }
    let frame = format!("{}\n", json!({"operation":"build-logs","attempt":attempt}));
    let text = command::run(EXECUTION, &[], Some(frame.as_bytes()), 5)
        .await
        .map_err(|_| Error::Refused(UNAVAILABLE))?;
    let mut reply: Value = serde_json::from_str(&text)?;
    if reply["ok"] != true {
        return Err(Error::Refused(UNAVAILABLE));
    }
    let result: Snapshot = serde_json::from_value(reply["value"].take())?;
    if result.attempt != attempt || result.stdout.len() > 65_536 || result.stderr.len() > 65_536 {
        return Err(Error::Refused(
            "The build diagnostics did not match the request.",
        ));
    }
    Ok(serde_json::to_value(result)?)
}
