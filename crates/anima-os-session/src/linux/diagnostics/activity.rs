//! Bounded lifecycle events from the system manager, without journal message text.
use crate::linux::{Error, Result, command, operator_activity::OperatorActivity, workloads};
use serde::Serialize;
use serde_json::{Value, json};

const UNITS: &[(&str, &str)] = &[
    ("anima-applications.service", "Hosted applications"),
    ("anima-daemon.service", "Node monitoring"),
    ("anima-execution-recovery.service", "Managed work recovery"),
    ("anima-link.service", "Private link"),
    ("anima-source-jobs.service", "Source jobs"),
    ("anima-update-health.service", "OS update health"),
    ("docker.service", "Docker"),
];

const TRANSITIONS: &[(&str, &str)] = &[
    ("7d4958e842da4a758f6c1cdc7b36dcc5", "Starting"),
    ("39f53479d3a045ac8e11786248231fbf", "Started"),
    ("be02cf6855d2428ba40df7e9d022f03d", "Could not start"),
    ("de5b426a63be47a7b6ac3eaac82e2f6f", "Stopping"),
    ("9d1aaa27d60140bd96365438aad20286", "Stopped"),
    ("d9b373ed55a64feb8242e02dbe79a49c", "Service failed"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    service: &'static str,
    transition: &'static str,
    at_ms: u64,
}

pub async fn read(operation: &str, history: &OperatorActivity) -> Result<Value> {
    match operation {
        "activity" => Ok(activity(history).await),
        "code-attention" => attention().await,
        _ => Err(Error::Refused("Unsupported activity operation.")),
    }
}

async fn activity(history: &OperatorActivity) -> Value {
    let (system, managed, actions) = tokio::join!(
        system_events(),
        workloads::managed(false),
        history.snapshot()
    );
    json!({
        "system": system.ok(),
        "actions": actions,
        "builds": managed.and_then(|value| value.get("builds").cloned()),
    })
}

async fn attention() -> Result<Value> {
    let output = command::run(
        "/run/current-system/sw/bin/anima-os-execution",
        &[],
        Some(b"{\"operation\":\"code-attention\"}\n"),
        8,
    )
    .await?;
    let mut reply: Value = serde_json::from_str(&output)?;
    if reply["ok"] != true || !reply["value"].is_object() {
        return Err(Error::Refused("Coding session status is unavailable."));
    }
    Ok(reply["value"].take())
}

async fn system_events() -> Result<Vec<Event>> {
    let mut matches = [
        "--boot",
        "--system",
        "--quiet",
        "--no-pager",
        "--lines=50",
        "--reverse",
        "--output=json",
        "--output-fields=UNIT,MESSAGE_ID",
        "_PID=1",
    ]
    .map(str::to_owned)
    .to_vec();
    matches.extend(UNITS.iter().map(|(unit, _)| format!("UNIT={unit}")));
    matches.extend(TRANSITIONS.iter().map(|(id, _)| format!("MESSAGE_ID={id}")));
    let args: Vec<_> = matches.iter().map(String::as_str).collect();
    let output = command::run("/run/current-system/sw/bin/journalctl", &args, None, 5).await?;
    Ok(output.lines().filter_map(project).collect())
}

fn project(line: &str) -> Option<Event> {
    let fields: Value = serde_json::from_str(line).ok()?;
    let unit = fields["UNIT"].as_str()?;
    let id = fields["MESSAGE_ID"].as_str()?;
    let timestamp: u64 = fields["__REALTIME_TIMESTAMP"].as_str()?.parse().ok()?;
    let (_, service) = UNITS.iter().find(|(code, _)| *code == unit)?;
    let (_, transition) = TRANSITIONS.iter().find(|(code, _)| *code == id)?;
    Some(Event {
        service,
        transition,
        at_ms: timestamp / 1000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_reply_projects_only_known_lifecycle_fields() {
        let mut record = json!({
            "UNIT": "anima-link.service",
            "MESSAGE_ID": "d9b373ed55a64feb8242e02dbe79a49c",
            "__REALTIME_TIMESTAMP": "1795518000000000",
            "MESSAGE": "private process output",
            "__CURSOR": "internal-journal-cursor",
        });
        let event = project(&record.to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            json!({"service":"Private link","transition":"Service failed","atMs":1_795_518_000_000_u64})
        );
        record["UNIT"] = json!("customer.service");
        assert!(project(&record.to_string()).is_none());
        record["UNIT"] = json!("anima-link.service");
        record["MESSAGE_ID"] = json!("unclassified");
        assert!(project(&record.to_string()).is_none());
    }
}
