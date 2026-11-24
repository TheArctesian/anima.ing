//! The model state of the node (Section 8); it holds no prompt, output, file name of a copy or address (SEC-C-010).

use chrono::{SecondsFormat, Utc};
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::machine::MachineSettings;
use daemon_core::model_work::names::{Backend, Runtime, Word as _};
use daemon_core::model_work::releases::Release;
use serde_json::{Value, json};

use std::collections::BTreeMap;

use uuid::Uuid;

use super::supervise::Servers;

const MAX_LISTED: usize = 16;

/// A folder of an unpack in progress starts with `.` and does not read, so it is not listed.
#[must_use]
pub fn installed(folders: &Folders) -> Vec<Release> {
    let mut found = Vec::new();
    for runtime in Runtime::ALL {
        let Ok(entries) = std::fs::read_dir(folders.runtimes.join(runtime.as_str())) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((version, backend)) = name.rsplit_once('-') else {
                continue;
            };
            let backend = Backend::parse(backend);
            if let (Some(backend), Some(_)) = (backend, Release::number(*runtime, version)) {
                found.push(Release {
                    runtime: *runtime,
                    version: version.to_owned(),
                    backend,
                });
            }
        }
    }
    found.sort_by(|a, b| (a.runtime.as_str(), &a.version).cmp(&(b.runtime.as_str(), &b.version)));
    found.truncate(MAX_LISTED);
    found
}

/// The members `models` of each server and `memory` of the loads (Section 11.1), when they read.
pub type Loads = Option<(BTreeMap<Uuid, Vec<Value>>, Vec<Value>)>;

#[must_use]
pub fn model_state(
    folders: &Folders,
    servers: &Servers,
    settings: &MachineSettings,
    loads: Loads,
) -> Value {
    let (models, memory) = loads.unwrap_or_default();
    let installed: Vec<Value> = installed(folders)
        .iter()
        .map(|release| json!({ "runtime": release.runtime.as_str(), "version": release.version, "backend": release.backend.as_str() }))
        .collect();
    let servers: Vec<Value> = servers
        .map
        .iter()
        .take(MAX_LISTED)
        .map(|(id, server)| {
            let listed = models.get(id).cloned().unwrap_or_default();
            // `loaded` stays for a hub that does not know `models`: the copies that are `ready` or `active`.
            let loaded: Vec<&Value> = listed
                .iter()
                .filter(|model| matches!(model["state"].as_str(), Some("ready" | "active")))
                .map(|model| &model["copy"])
                .collect();
            json!({
                "server": id, "runtime": server.runtime, "state": server.state.as_str(),
                "restarts": server.record.restarts(), "loaded": loaded, "models": listed,
                "inProgress": 0, "queued": 0,
                "tokensPerSecond": null, "memoryBytes": 0,
                "tempBytes": super::folders::bytes_below(&folders.server(*id).join("tmp")),
            })
        })
        .collect();
    json!({
        "at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "installed": installed,
        "servers": servers,
        "copies": [],
        "cacheBytes": 0,
        "settings": settings.to_json(),
        "memory": memory,
    })
}
