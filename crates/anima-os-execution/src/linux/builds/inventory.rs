//! Console projection excludes source, logs and build metadata supplied by workers.
use super::{
    super::{Error, Result, control::inventory},
    output::progress,
    store,
};
use serde_json::{Value, json};
use std::{fs, path::Path};
use uuid::Uuid;

pub fn list(usage: bool) -> Result<Value> {
    let mut entries = Vec::new();
    let mut truncated = false;
    if Path::new(store::ROOT).try_exists()? {
        for entry in fs::read_dir(store::ROOT)? {
            let name = entry?.file_name();
            if name == "locks" {
                continue;
            }
            if entries.len() == 256 {
                truncated = true;
                break;
            }
            let id = name
                .to_str()
                .and_then(|name| Uuid::parse_str(name).ok())
                .ok_or(Error::Refused(
                    "Build inventory contains an invalid identity.",
                ))?;
            let state = store::required(id)?;
            if state.phase == "deleted" {
                continue;
            }
            let progress = progress::observe(&store::base(id).join("work"));
            let progress_available = progress.is_ok();
            let mut entry = json!({"attempt":state.id,"workspace":state.workspace,
                "generation":state.generation,"phase":state.phase,"resources":state.resources,
                "strategy":state.strategy,"observation":"receipt","workerProgress":progress.ok().flatten(),
                "workerProgressAvailable":progress_available});
            if usage {
                let sample = match state.phase.as_str() {
                    "launching" | "running" | "cancelling" | "unknown" => {
                        inventory::usage::read(&store::unit(state.id))
                    }
                    _ => None,
                };
                entry["usage"] = json!(sample);
            }
            entries.push(entry);
        }
    }
    entries.sort_by_key(|entry| entry["attempt"].as_str().unwrap_or_default().to_owned());
    Ok(json!({"entries":entries,"truncated":truncated}))
}
