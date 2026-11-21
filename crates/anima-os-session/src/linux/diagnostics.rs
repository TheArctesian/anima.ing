use super::{Error, Result, command::run, network};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Default)]
pub struct Diagnostic {
    request: Option<Uuid>,
    operation: Option<Uuid>,
    revision: u64,
}
impl Diagnostic {
    pub async fn read(&mut self) -> Result<Value> {
        let mut result = json!({"network":run("nmcli", &["-t","-f","STATE,CONNECTIVITY","general"],None,10).await?,
            "radio":run("rfkill", &["--json"],None,5).await?,"root":run("findmnt", &["-n","-o","OPTIONS","/"],None,5).await?,
            "monitoring":network::unit("anima-daemon").await,"wifi_backend":network::unit("wpa_supplicant").await});
        let Ok(program) = std::env::var("ANIMA_OS_CONTROL") else {
            return Ok(result);
        };
        let first = self.request.is_none();
        self.begin(first);
        let request = self.request.get_or_insert_with(Uuid::now_v7).to_string();
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut action = if first { "job-start" } else { "job-get" };
        loop {
            let reply = super::command::json_reply(&program, &[action, &request], 4).await?;
            if reply["version"] != 2 {
                return Err(Error::Refused(
                    "The diagnostic service returned an incompatible response.",
                ));
            }
            if reply["result"] == "error" {
                if reply["value"] == "not_found" {
                    self.request = None;
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
            self.observe(value)?;
            match value["state"].as_str() {
                Some("succeeded") => {
                    merge(&mut result, &value["result"])?;
                    self.request = None;
                    return Ok(result);
                }
                Some("cancelled" | "interrupted") => {
                    self.request = None;
                    return Err(Error::Refused("The check was interrupted. Run it again."));
                }
                Some("queued" | "running") if Instant::now() < deadline => {}
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

    fn begin(&mut self, first: bool) {
        if first {
            self.operation = None;
            self.revision = 0;
        }
    }

    fn observe(&mut self, value: &Value) -> Result<()> {
        let operation = value["id"]
            .as_str()
            .and_then(|id| id.parse::<Uuid>().ok())
            .ok_or(Error::Refused("Invalid diagnostic operation identity."))?;
        let revision = value["revision"]
            .as_u64()
            .ok_or(Error::Refused("Invalid diagnostic operation revision."))?;
        if self.operation.is_some_and(|id| id != operation) || revision < self.revision {
            return Err(Error::Refused(
                "The diagnostic operation changed unexpectedly.",
            ));
        }
        if !matches!(
            value["state"].as_str(),
            Some("queued" | "running" | "succeeded" | "cancelled" | "interrupted")
        ) {
            return Err(Error::Refused("Invalid diagnostic operation state."));
        }
        self.operation = Some(operation);
        self.revision = revision;
        Ok(())
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
