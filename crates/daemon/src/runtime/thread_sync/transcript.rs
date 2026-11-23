use agent_threads::control::{Action, Change, Outcome, Update};
use agent_threads::drive::DriveEvent;
use daemon_core::wire::{
    MAX_THREAD_ENTRIES, MAX_THREAD_ENTRY, ThreadEntry, ThreadSummary, ThreadUpload,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{Target, runner};
use crate::{
    config::{agent_usage::UsageSettings, home::Home, private_file},
    hub::answer::Answer,
    token_store::Credential,
};

const SETTINGS: &str = "thread-sync.json";
const JOURNAL_ROOM: usize = 512 * 1024;
const PAGE: usize = 64;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    share_transcripts: bool,
}

#[derive(Default, Deserialize, Serialize)]
struct Journal {
    observed: u64,
    accepted: i64,
    pending: Vec<ThreadEntry>,
}

pub(super) async fn publish(
    home: &Home,
    target: &Target,
    rows: &[ThreadSummary],
    (policy, permitted): (&UsageSettings, &[Uuid]),
) {
    if !enabled(home) {
        discard_unshared(home, target.device, &[]);
        return;
    }
    let tokens = target.tokens.clone();
    let Ok(Ok(Some(credential))) = tokio::task::spawn_blocking(move || tokens.load()).await else {
        return;
    };
    if credential.hub != target.origin {
        return;
    }
    for row in rows
        .iter()
        .filter(|row| permitted.binary_search(&row.runner_id).is_ok())
    {
        if !authorized(home, policy) {
            return;
        }
        send_thread(home, target, row, (&credential, policy)).await;
    }
}

async fn send_thread(
    home: &Home,
    target: &Target,
    row: &ThreadSummary,
    (credential, policy): (&Credential, &UsageSettings),
) {
    let name = journal_name(target.device, row.runner_id);
    let Some(mut journal) = read(home, &name) else {
        return;
    };
    if journal.pending.is_empty() {
        let updates = earlier(row.runner_id, journal.observed).await;
        if !authorized(home, policy) {
            return;
        }
        journal.record(updates);
    }
    if journal.pending.is_empty() || !save(home, &name, &journal) || !authorized(home, policy) {
        return;
    }
    let upload = ThreadUpload {
        device: target.device,
        summary: row.clone(),
        entries: journal.pending.clone(),
    };
    let Answer::Accepted(receipt) = target.hub.upload_thread(&credential.token, &upload).await
    else {
        return;
    };
    journal.accepted = journal.accepted.max(receipt.last_sequence);
    journal
        .pending
        .retain(|entry| entry.sequence > receipt.last_sequence);
    if !save(home, &name, &journal) {
        tracing::warn!("the thread upload receipt could not be stored");
    }
}

async fn earlier(thread: Uuid, observed: u64) -> Vec<Update> {
    let env = |name: &str| std::env::var(name).ok();
    let mut before = u64::MAX;
    let mut pages = Vec::new();
    while let Some(Outcome::Earlier { updates }) = runner::call(
        env,
        Action::Earlier {
            thread,
            before,
            most: PAGE,
        },
    )
    .await
    {
        let Some(first) = updates.first() else { break };
        if first.seq >= before {
            return Vec::new();
        }
        before = first.seq;
        pages.push(updates);
        if before <= observed {
            break;
        }
    }
    pages
        .into_iter()
        .rev()
        .flatten()
        .filter(|update| update.seq > observed)
        .collect()
}

impl Journal {
    fn record(&mut self, updates: Vec<Update>) {
        for update in updates {
            self.observed = self.observed.max(update.seq);
            let Some(content) = content(update.change) else {
                continue;
            };
            let sequence = self.accepted + i64::try_from(self.pending.len()).unwrap_or(0) + 1;
            self.pending.push(ThreadEntry { sequence, content });
            if self.pending.len() == MAX_THREAD_ENTRIES {
                break;
            }
        }
    }
}

fn content(change: Change) -> Option<Value> {
    let value = match change {
        Change::State { state, .. } => json!({"change": "state", "state": state}),
        Change::Said {
            event: DriveEvent::Account { .. } | DriveEvent::Reply { .. } | DriveEvent::Models { .. },
        } => return None,
        other => serde_json::to_value(other).ok()?,
    };
    let mut value = value;
    redact(&mut value);
    if serde_json::to_vec(&value).ok()?.len() > MAX_THREAD_ENTRY {
        return Some(json!({"change": "omitted", "reason": "entry_size"}));
    }
    Some(value)
}

fn redact(value: &mut Value) {
    match value {
        Value::String(text) => *text = redaction::redact_document(text),
        Value::Array(rows) => rows.iter_mut().for_each(redact),
        Value::Object(fields) => fields.values_mut().for_each(redact),
        _ => (),
    }
}

fn enabled(home: &Home) -> bool {
    private_file::read(&home.dir().join(SETTINGS))
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
        .is_some_and(|settings| settings.share_transcripts)
}

fn journal_name(device: Uuid, thread: Uuid) -> String {
    format!("thread-sync-{device}-{thread}.json")
}

fn read(home: &Home, name: &str) -> Option<Journal> {
    let bytes = private_file::read_within(&home.dir().join(name), JOURNAL_ROOM).ok()?;
    bytes.as_ref().map_or_else(
        || Some(Journal::default()),
        |bytes| serde_json::from_slice(bytes).ok(),
    )
}

fn save(home: &Home, name: &str, journal: &Journal) -> bool {
    serde_json::to_vec(journal)
        .is_ok_and(|bytes| private_file::write(home.dir(), name, &bytes).is_ok())
}

fn authorized(home: &Home, policy: &UsageSettings) -> bool {
    enabled(home) && UsageSettings::read(home).ok().as_ref() == Some(policy)
}

pub(super) fn discard_unshared(home: &Home, device: Uuid, permitted: &[Uuid]) {
    let prefix = format!("thread-sync-{device}-");
    let Ok(paths) = std::fs::read_dir(home.dir()) else {
        return;
    };
    for path in paths.filter_map(Result::ok) {
        let name = path.file_name();
        let Some(name) = name.to_str() else { continue };
        let id = name
            .strip_prefix(&prefix)
            .and_then(|name| name.strip_suffix(".json"))
            .and_then(|id| id.parse::<Uuid>().ok());
        if id.is_some_and(|id| permitted.binary_search(&id).is_err()) {
            discard(home, name);
        }
    }
}

fn discard(home: &Home, name: &str) {
    let Some(mut journal) = read(home, name) else {
        return;
    };
    if journal.pending.is_empty() {
        return;
    }
    for entry in &mut journal.pending {
        entry.content = json!({"change": "omitted", "reason": "sharing_disabled"});
    }
    if !save(home, name, &journal) {
        tracing::warn!("unshared thread entries could not be cleared");
    }
}
