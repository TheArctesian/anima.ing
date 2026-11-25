mod transcript;

pub(crate) use transcript::enabled as transcripts_enabled;

use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

use agent_threads::control::{Action, Outcome, RunState, ThreadRow};
use agent_threads::drive::Program;
use chrono::Utc;
use daemon_core::wire::{ThreadSummaries, ThreadSummary};
use tokio::{sync::mpsc, time::MissedTickBehavior};
use uuid::Uuid;

use super::agent_usage::runner;
use crate::{
    config::origin::Origin,
    config::{agent_usage::UsageSettings, home::Home, private_file},
    hub::HubClient,
    push::Event,
    readers::agent_sessions::{shared_repository, user_home},
    remote_code::Store,
    token_store::TokenStore,
};

const REVISION: &str = "thread-summary-revision";

pub(crate) struct Target {
    pub(crate) hub: Arc<HubClient>,
    pub(crate) device: Uuid,
    pub(crate) origin: Origin,
    pub(crate) tokens: TokenStore,
}

pub(super) async fn publish(home: Home, events: mpsc::Sender<Event>, target: Target) {
    let env = |name: &str| std::env::var(name).ok();
    let Some(user) = user_home(env) else { return };
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last: Option<Vec<ThreadSummary>> = None;
    let mut previous_policy = None;
    loop {
        ticks.tick().await;
        let Ok(settings) = UsageSettings::read(&home) else {
            last = None;
            transcript::discard_unshared(&home, target.device, &[]);
            if events.send(Event::ThreadSummaries(None)).await.is_err() {
                return;
            }
            continue;
        };
        if settings.off {
            last = None;
            transcript::discard_unshared(&home, target.device, &[]);
            if events.send(Event::ThreadSummaries(None)).await.is_err() {
                return;
            }
            continue;
        }
        if previous_policy.as_ref() != Some(&settings) {
            last = None;
            previous_policy = Some(settings.clone());
        }
        let Some(Outcome::Threads { threads, .. }) = runner::call(env, Action::List).await else {
            continue;
        };
        let mut rows: Vec<_> = threads
            .iter()
            .filter_map(|row| summary(row, &user, &settings))
            .collect();
        rows.sort_unstable_by_key(|row| row.runner_id);
        let permitted: Vec<_> = rows.iter().map(|row| row.runner_id).collect();
        transcript::discard_unshared(&home, target.device, &permitted);
        let priority = Store::at(&home)
            .publication_requests(target.device, &target.origin)
            .unwrap_or_default();
        if let Some(batch) = revised(&home, rows, last.as_deref(), &priority) {
            last = Some(batch.summaries.clone());
            if !offer(&events, batch, &settings, priority).await {
                return;
            }
        }
        if let Some(rows) = &last {
            transcript::publish(&home, &target, rows, (&settings, &permitted)).await;
        }
    }
}

async fn offer(
    events: &mpsc::Sender<Event>,
    batch: ThreadSummaries,
    settings: &UsageSettings,
    priority: BTreeMap<Uuid, i64>,
) -> bool {
    events
        .send(Event::ThreadSummaries(Some((
            batch,
            settings.clone(),
            priority.into_keys().collect(),
        ))))
        .await
        .is_ok()
}

fn summary(row: &ThreadRow, user: &Path, settings: &UsageSettings) -> Option<ThreadSummary> {
    let repository = shared_repository(user, &row.folder, settings)?.name;
    let program = match row.program {
        Program::ClaudeCode => "claude_code",
        Program::Codex => "codex",
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
        program: program.to_owned(),
        state: state.to_owned(),
        title: row
            .title
            .chars()
            .filter(|ch| !ch.is_control())
            .take(80)
            .collect(),
        started_at: row.started,
        activity_at: row.started,
        revision: 0,
        repository,
    })
}

fn revised(
    home: &Home,
    rows: Vec<ThreadSummary>,
    last: Option<&[ThreadSummary]>,
    priority: &BTreeMap<Uuid, i64>,
) -> Option<ThreadSummaries> {
    let old = last.unwrap_or_default();
    if last.is_some()
        && rows.len() == old.len()
        && rows
            .iter()
            .zip(old)
            .all(|(row, old)| already_current(row, old, priority))
    {
        return None;
    }
    let revision = revision(home).or_else(|| {
        tracing::warn!("thread summary revision could not be stored");
        None
    })?;
    let now = Utc::now();
    let count = u32::try_from(rows.len()).ok()?;
    let summaries = rows
        .into_iter()
        .map(|row| {
            old.binary_search_by_key(&row.runner_id, |old| old.runner_id)
                .ok()
                .map(|at| &old[at])
                .filter(|old| already_current(&row, old, priority))
                .cloned()
                .unwrap_or_else(|| ThreadSummary {
                    revision,
                    activity_at: row.started_at.max(now),
                    ..row
                })
        })
        .collect();
    Some(ThreadSummaries {
        count,
        count_revision: revision,
        summaries,
    })
}

fn already_current(
    row: &ThreadSummary,
    old: &ThreadSummary,
    priority: &BTreeMap<Uuid, i64>,
) -> bool {
    priority
        .get(&row.runner_id)
        .is_none_or(|minimum| old.revision >= *minimum)
        && same(row, old)
}

fn same(row: &ThreadSummary, old: &ThreadSummary) -> bool {
    row.runner_id == old.runner_id
        && row.state == old.state
        && row.title == old.title
        && row.program == old.program
        && row.repository == old.repository
        && row.started_at == old.started_at
}

pub(crate) fn revision(home: &Home) -> Option<i64> {
    use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
    let writing = open(
        home.dir().join("thread-summary-revision.lock"),
        OFlags::CREATE | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .ok()?;
    flock(&writing, FlockOperation::NonBlockingLockExclusive).ok()?;

    let stored = private_file::read(&home.dir().join(REVISION)).ok()?;
    let previous = stored.as_ref().map_or(Some(0_i64), |bytes| {
        std::str::from_utf8(bytes).ok()?.parse().ok()
    })?;
    let revision = previous.checked_add(1)?.max(Utc::now().timestamp_micros());
    if revision > (1_i64 << 53) - 1 {
        return None;
    }
    private_file::write(home.dir(), REVISION, revision.to_string().as_bytes()).ok()?;
    Some(revision)
}

pub(crate) fn remote_publishable(home: &Home, row: &ThreadRow) -> bool {
    let Some(user) = user_home(|name| std::env::var(name).ok()) else {
        return false;
    };
    let Ok(settings) = UsageSettings::read(home) else {
        return false;
    };
    transcripts_enabled(home) && summary(row, &user, &settings).is_some()
}
