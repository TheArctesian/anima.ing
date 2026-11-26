//! Local administration only. The runner projects counts before crossing the workspace boundary.
use super::{exchange, workspace};
use crate::linux::{Error, Result, command};
use agent_threads::control::{Action, Call, Outcome};
use serde::Serialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Semaphore, task::JoinSet, time::Instant};
use uuid::Uuid;
use workspace::record::{self, Phase};

const MAX_TARGETS: usize = 128;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AwaitingResponse {
    workspace: Uuid,
    generation: u64,
    waiting_threads: usize,
}

pub(in crate::linux) async fn read() -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let containers = tokio::time::timeout(
        Duration::from_secs(1),
        command::docker(&[
            "ps",
            "--filter",
            "label=ing.anima.workspace",
            "--format",
            "{{.Names}}\t{{.Label \"ing.anima.workspace\"}}",
        ]),
    )
    .await
    .map_err(|_| Error::Refused("Running workspaces could not be checked."))??;
    let (targets, incomplete) = targets(&containers);
    if !targets.is_empty() && anima_os_maintenance::pressure::require_clear().is_err() {
        return Ok(json!({"entries":[],"complete":false}));
    }
    let (entries, unavailable) = collect(targets, deadline).await;
    Ok(json!({"entries":entries,"complete":!incomplete && !unavailable}))
}

fn targets(containers: &str) -> (Vec<(Uuid, u64)>, bool) {
    let mut targets = Vec::new();
    let mut incomplete = false;
    for (index, line) in containers.lines().enumerate() {
        if index == MAX_TARGETS {
            return (targets, true);
        }
        match target(line) {
            Ok(Some(target)) => targets.push(target),
            Ok(None) => (),
            Err(_) => incomplete = true,
        }
    }
    (targets, incomplete)
}

fn target(line: &str) -> Result<Option<(Uuid, u64)>> {
    let (name, workspace) = line
        .split_once('\t')
        .and_then(|(name, label)| Some((name, Uuid::parse_str(label).ok()?)))
        .ok_or(Error::Refused("Workspace runtime identity is unavailable."))?;
    let state = record::required(workspace)?;
    if state.container() != name {
        return Err(Error::Refused("Workspace runtime identity changed."));
    }
    if state.principal.is_none() {
        return Ok(None);
    }
    if state.phase != Phase::Running {
        return Err(Error::Refused("Workspace state needs reconciliation."));
    }
    Ok(Some((workspace, state.generation)))
}

async fn collect(targets: Vec<(Uuid, u64)>, deadline: Instant) -> (Vec<AwaitingResponse>, bool) {
    let slots = Arc::new(Semaphore::new(4));
    let mut probes = JoinSet::new();
    for (workspace, generation) in targets {
        let slots = slots.clone();
        probes.spawn(async move {
            let _slot = slots
                .acquire_owned()
                .await
                .map_err(|_| Error::Refused("Workspace probes are unavailable."))?;
            probe(workspace, generation).await
        });
    }
    let mut entries = Vec::new();
    let mut unavailable = false;
    while let Ok(Some(result)) = tokio::time::timeout_at(deadline, probes.join_next()).await {
        match result {
            Ok(Ok(Some(entry))) => entries.push(entry),
            Ok(Ok(None)) => (),
            _ => unavailable = true,
        }
    }
    entries.sort_unstable_by_key(|entry| entry.workspace);
    (entries, unavailable || !probes.is_empty())
}

async fn probe(workspace: Uuid, generation: u64) -> Result<Option<AwaitingResponse>> {
    let container = workspace::attention_target(workspace, generation).await?;
    let reply = exchange::probe(
        &container,
        Call {
            id: 1,
            action: Action::ManagedAttention,
        },
    )
    .await?;
    let Outcome::ManagedAttention { waiting_threads } = reply else {
        return Err(Error::Refused(
            "Workspace attention information is unavailable.",
        ));
    };
    Ok((waiting_threads > 0).then_some(AwaitingResponse {
        workspace,
        generation,
        waiting_threads,
    }))
}
