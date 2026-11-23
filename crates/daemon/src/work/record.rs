//! The machine's own log of the work that it took (K10 Section 9, SF-012): one entry for each
//! item, also each refused one, written before the state `preparing`. An entry holds no secret,
//! and no member of the item that this file does not name.

use chrono::{DateTime, Utc};
use daemon_core::wire::WorkJob;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Taken,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub at: DateTime<Utc>,
    pub id: Uuid,
    pub attempt: u32,
    #[serde(rename = "type")]
    pub work_type: Option<String>,
    pub kind: Option<String>,
    pub project: Option<String>,
    pub repository: Option<String>,
    pub commit: Option<String>,
    pub recipe: Option<String>,
    pub runner_version: Option<String>,
    pub runner_sha256: Option<String>,
    /// The purposes of the secrets, never a value.
    pub secrets: Vec<String>,
    pub outcome: Outcome,
    pub cause: Option<&'static str>,
}

/// A refused item can hold a value of any length.
const MOST: usize = 128;

fn short(value: Option<&Value>) -> Option<String> {
    Some(value?.as_str()?.chars().take(MOST).collect())
}

/// Each value is read from the answer as it came, since the machine may have refused it.
#[must_use]
pub fn entry(
    item: &Value,
    (id, attempt): (Uuid, u32),
    (outcome, cause): (Outcome, Option<&'static str>),
    at: DateTime<Utc>,
) -> Entry {
    let parameters = item.get("parameters");
    let parameter = |name: &str| short(parameters.and_then(|all| all.get(name)));
    let names = |name: &str| short(item.get("repository").and_then(|all| all.get(name)));
    let secrets = item.get("secrets").and_then(Value::as_array);
    Entry {
        at,
        id,
        attempt,
        work_type: short(item.get("type")),
        kind: short(item.get("kind")),
        project: short(item.get("project")),
        repository: names("owner")
            .zip(names("name"))
            .map(|(owner, name)| format!("{owner}/{name}")),
        commit: parameter("commit"),
        recipe: parameter("recipe"),
        runner_version: parameter("runnerVersion"),
        runner_sha256: parameter("runnerSha256"),
        secrets: secrets.map_or_else(Vec::new, |all| {
            all.iter()
                .filter_map(|each| short(Some(each)))
                .take(8)
                .collect()
        }),
        outcome,
        cause,
    }
}

#[must_use]
pub fn taken(job: &WorkJob, at: DateTime<Utc>) -> Entry {
    let item = serde_json::to_value(job).unwrap_or(Value::Null);
    entry(&item, (job.id, job.attempt), (Outcome::Taken, None), at)
}
