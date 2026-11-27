//! Console projection of workspace lifecycle receipts, without coding content.
use super::{Error, Result, record};
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

mod docker;
mod storage;

pub fn list() -> Result<Value> {
    let (records, truncated) = records()?;
    Ok(render(records, truncated, None, None))
}

pub(in crate::linux) async fn with_usage() -> Result<Value> {
    let (records, truncated) = records()?;
    let running = async {
        if records.iter().any(has_runtime) {
            docker::Running::observe().await
        } else {
            docker::Running::default()
        }
    };
    let accounting = async {
        if records.is_empty() {
            None
        } else {
            storage::Accounting::observe().await
        }
    };
    let (running, accounting) = tokio::join!(running, accounting);
    Ok(render(
        records,
        truncated,
        Some(&running),
        accounting.as_ref(),
    ))
}

fn render(
    mut records: Vec<record::Record>,
    truncated: bool,
    running: Option<&docker::Running>,
    accounting: Option<&storage::Accounting>,
) -> Value {
    records.sort_by_key(|record| record.workspace);
    let entries: Vec<_> = records
        .into_iter()
        .map(|state| {
            let mut entry = json!({"workspace":state.workspace,"generation":state.generation,
                "phase":state.phase,"resources":state.resources,"storageBytes":state.storage_bytes,
                "observation":"receipt"});
            if let Some(running) = running {
                let sample = if has_runtime(&state) {
                    running.usage(&state)
                } else {
                    None
                };
                entry["usage"] = json!(sample);
                entry["storageUsageBytes"] =
                    json!(accounting.and_then(|value| value.referenced_bytes(&state)));
            }
            entry
        })
        .collect();
    json!({"entries":entries,"truncated":truncated})
}

fn records() -> Result<(Vec<record::Record>, bool)> {
    let mut records = Vec::new();
    for entry in fs::read_dir("/persist/anima-workspaces")? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        if records.len() == 256 {
            return Ok((records, true));
        }
        let id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(Error::Refused(
                "Workspace inventory contains an invalid identity.",
            ))?;
        let state = record::required(id)?;
        if state.phase != record::Phase::Deleted {
            records.push(state);
        }
    }
    Ok((records, false))
}

fn has_runtime(record: &record::Record) -> bool {
    use record::Phase::{Running, Starting, Stopping, Unknown};
    record.run.is_some() && matches!(record.phase, Running | Starting | Stopping | Unknown)
}
