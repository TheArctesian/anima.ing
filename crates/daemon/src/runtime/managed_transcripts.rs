//! Transcript publishing only for Code launches acknowledged by the managed command owner.
mod registry;
mod scheduling;
use super::{HubTarget, managed_code::broker};
use crate::{config::home::Home, hub::answer::Answer};
use agent_threads::{
    control::RunState,
    drive::Program,
    managed::{LocalRequest, Report, Snapshot, ThreadSnapshot},
};
use chrono::Utc;
use daemon_core::wire::{MAX_THREAD_ENTRIES, ThreadSummary, ThreadUpload};
use registry::{Journal, Registry};
use serde_json::json;
use std::{path::Path, time::Duration};
use uuid::Uuid;

pub(super) fn register(outbox: &Path, origin: &str, device: Uuid, report: &Report) -> bool {
    let Some(agent_threads::control::Outcome::Started { thread, .. }) = &report.result else {
        return true;
    };
    let Some(registry) = outbox.parent().and_then(Registry::open) else {
        return false;
    };
    registry.register(&Journal {
        origin: origin.to_owned(),
        device,
        workspace: report.workspace,
        generation: report.generation,
        thread: *thread,
        observed: 0,
        terminal: false,
        open: Vec::new(),
        current: Vec::new(),
        accepted: 0,
        revision: 0,
        summary: None,
        pending: Vec::new(),
    })
}
pub(super) async fn run(home: Home, target: HubTarget) {
    if std::env::var("ANIMA_MANAGED_CODE").as_deref() != Ok("1") {
        return;
    }
    let Some(registry) = Registry::open(home.dir()) else {
        return;
    };
    tokio::join!(
        scheduling::worker(&registry, &target, 0),
        scheduling::worker(&registry, &target, 1),
        scheduling::worker(&registry, &target, 2),
        scheduling::worker(&registry, &target, 3),
    );
}

async fn publish(registry: &Registry, target: &HubTarget, mut journal: Journal) -> Duration {
    if journal.origin != target.origin.as_text() || journal.device != target.device {
        return Duration::from_secs(5);
    }
    let tokens = target.tokens.clone();
    let Ok(Ok(Some(credential))) = tokio::task::spawn_blocking(move || tokens.load()).await else {
        return Duration::from_secs(5);
    };
    if credential.hub != target.origin || credential.expires_at <= Utc::now() {
        return Duration::from_secs(5);
    }
    if journal.pending.is_empty() && !collect(&mut journal).await {
        return Duration::from_secs(5);
    }
    if !registry.save(&journal) {
        return Duration::from_secs(5);
    }
    if journal.pending.is_empty() {
        if journal.terminal {
            let _ = registry.archive(&journal);
        }
        return cadence(&journal);
    }
    let Some(upload) = upload(target.device, &journal) else {
        return Duration::from_secs(5);
    };
    let Answer::Accepted(receipt) = target.hub.upload_thread(&credential.token, &upload).await
    else {
        return Duration::from_secs(5);
    };
    let sent = upload
        .entries
        .last()
        .map_or(journal.accepted, |entry| entry.sequence);
    if receipt.last_sequence < journal.accepted || receipt.last_sequence > sent {
        return Duration::from_secs(5);
    }
    journal.accepted = receipt.last_sequence;
    journal
        .pending
        .retain(|entry| entry.sequence > receipt.last_sequence);
    if !registry.save(&journal) {
        return Duration::from_secs(5);
    }
    if journal.terminal && journal.pending.is_empty() {
        let _ = registry.archive(&journal);
    }
    cadence(&journal)
}

fn upload(device: Uuid, journal: &Journal) -> Option<ThreadUpload> {
    let upload = ThreadUpload {
        device,
        summary: journal.summary.clone()?,
        entries: journal
            .pending
            .iter()
            .take(MAX_THREAD_ENTRIES)
            .cloned()
            .collect(),
    };
    upload.fits().then_some(upload)
}

fn cadence(journal: &Journal) -> Duration {
    if !journal.pending.is_empty() {
        return Duration::from_millis(100);
    }
    match journal
        .summary
        .as_ref()
        .map(|summary| summary.state.as_str())
    {
        Some("stopped") => Duration::from_secs(60),
        Some("idle") => Duration::from_secs(5),
        _ => Duration::from_secs(1),
    }
}

async fn collect(journal: &mut Journal) -> bool {
    let Some(value) = broker::call(&LocalRequest::Snapshot {
        workspace: journal.workspace,
        generation: journal.generation,
        thread: journal.thread,
        after: journal.observed,
    })
    .await
    else {
        return false;
    };
    let Ok(snapshot) = serde_json::from_value::<Snapshot>(value) else {
        return false;
    };
    if snapshot.row.id != journal.thread {
        return false;
    }
    let had_updates = !snapshot.updates.is_empty();
    journal.record(snapshot.updates);
    let row = snapshot.row;
    journal.terminal = !had_updates && row.state == RunState::Stopped && row.open.is_empty();
    if (had_updates || journal.current != snapshot.current || journal.open != row.open)
        && journal.pending.len() + snapshot.current.len() < 128
    {
        for value in &snapshot.current {
            journal.append(value.clone());
        }
        journal.append(json!({"change":"open_requests","requests":row.open}));
        journal.current = snapshot.current;
        journal.open.clone_from(&row.open);
    }
    let Some(mut summary) = summary(&row, journal.revision) else {
        return false;
    };
    let changed = journal
        .summary
        .as_ref()
        .is_none_or(|old| old.state != summary.state || old.title != summary.title);
    if changed && journal.pending.is_empty() {
        journal.append(json!({"change":"state","state":summary.state}));
    }
    if !journal.pending.is_empty() {
        journal.revision = journal.revision.saturating_add(1);
        summary.revision = journal.revision;
        journal.summary = Some(summary);
    }
    true
}
fn summary(row: &ThreadSnapshot, revision: i64) -> Option<ThreadSummary> {
    let program = match row.program {
        Program::Codex => "codex",
        Program::ClaudeCode => "claude_code",
        Program::Grok => return None,
    };
    let state = match row.state {
        RunState::Starting => "starting",
        RunState::Working => "working",
        RunState::NeedsYou => "needs_you",
        RunState::Idle => "idle",
        RunState::Stopped => "stopped",
        RunState::Unknown => "unknown",
    };
    Some(ThreadSummary {
        runner_id: row.id,
        program: program.into(),
        state: state.into(),
        title: redaction::redact_document(&row.title)
            .chars()
            .filter(|ch| !ch.is_control())
            .take(80)
            .collect(),
        started_at: row.started,
        activity_at: row.started.max(Utc::now()),
        revision,
        repository: None,
    })
}
