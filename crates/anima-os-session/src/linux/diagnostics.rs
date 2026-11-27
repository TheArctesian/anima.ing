pub(crate) mod activity;
mod pending;

use super::{
    Error, Result,
    command::{json_reply, run, unit_state},
};
use pending::Pending;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub async fn read() -> Result<Value> {
    let mut result = json!({"network":run("nmcli", &["-t","-f","STATE,CONNECTIVITY","general"],None,10).await?,
        "radio":run("rfkill", &["--json"],None,5).await?,"root":run("findmnt", &["-n","-o","OPTIONS","/"],None,5).await?,
        "monitoring":unit_state("anima-daemon").await,"wifi_backend":unit_state("wpa_supplicant").await});
    let Ok(program) = std::env::var("ANIMA_OS_CONTROL") else {
        return Ok(result);
    };
    let (mut pending, mut action) = Pending::open()?;
    let request = pending.request().to_string();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let reply = json_reply(&program, &[action, &request], 4).await?;
        if reply["version"] != 2 {
            return Err(Error::Refused(
                "The diagnostic service returned an incompatible response.",
            ));
        }
        if reply["result"] == "error" {
            if reply["value"] == "not_found" {
                if !pending.has_observation() && Instant::now() < deadline {
                    action = "job-get";
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                pending.finish()?;
                return Err(Error::Refused(
                    "This diagnostic check is no longer available. Run it again.",
                ));
            }
            return Err(Error::Refused(
                "Diagnostics are unavailable. Retry to reconnect to the same check.",
            ));
        }
        let value = &reply["value"];
        if reply["result"] != "operation" || value["request_id"] != request {
            return Err(Error::Refused(
                "The diagnostic service returned an incompatible operation.",
            ));
        }
        match pending.observe(value)? {
            "succeeded" => {
                merge(&mut result, &value["result"])?;
                pending.finish()?;
                return Ok(result);
            }
            "cancelled" | "interrupted" => {
                pending.finish()?;
                return Err(Error::Refused("The check was interrupted. Run it again."));
            }
            "queued" | "running" if Instant::now() < deadline => {}
            _ => {
                return Err(Error::Refused(
                    "Diagnostics are still running. Retry to reconnect.",
                ));
            }
        }
        action = "job-get";
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn merge(result: &mut Value, diagnostic: &Value) -> Result<()> {
    for key in ["monitoring", "private_link"] {
        let state = diagnostic[key]
            .as_str()
            .filter(|state| {
                [
                    "active",
                    "inactive",
                    "failed",
                    "activating",
                    "deactivating",
                    "unknown",
                ]
                .contains(state)
            })
            .ok_or(Error::Refused("Invalid diagnostic result."))?;
        result[key] = json!(state);
    }
    Ok(())
}
