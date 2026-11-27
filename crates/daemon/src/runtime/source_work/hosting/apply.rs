//! Fetch an apply's one-use inputs, then supervise its fenced local execution.
mod lease;

use std::time::Duration;

use anima_applications_wire::{Health, Reply};
use chrono::Utc;
use daemon_core::wire::{
    ApplicationApply, COMPOSE_FILE, ENVIRONMENT_FILE, Held, StopReason, WorkReport, WorkState,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio::{
    io::AsyncWriteExt as _,
    net::UnixStream,
    time::{self, Instant},
};

use crate::{
    applications::{self, ApplyFiles, Client, apply_failure},
    hub::{TIMEOUT, answer::Answer, work::ReleasedSecret},
    model::fetch::hex,
};

use super::{
    State,
    execution::{Authority, relinquished},
    reports, write,
};
use lease::Lease;

enum End {
    Refused(&'static str),
    Failed(&'static str),
    Applied(Health),
    Stopped(StopReason),
    Expired,
    Lost,
}

struct Work {
    parameters: ApplicationApply,
    lifetime: Duration,
    lease: daemon_core::wire::Lease,
}

impl Work {
    fn read(item: &Value) -> Option<Self> {
        let parameters: ApplicationApply =
            serde_json::from_value(item["parameters"].clone()).ok()?;
        if parameters.application.is_nil()
            || item["parameters"]["application"].as_str()? != parameters.application.to_string()
            || parameters.keep > 1
            || !is_hash(&parameters.file_sha256)
            || !parameters.environment_sha256.as_deref().is_none_or(is_hash)
            || item["project"] != Value::Null
            || item.get("repository").is_some()
        {
            return None;
        }
        let purposes = item["secrets"].as_array()?;
        let required: &[&str] = if parameters.environment_sha256.is_some() {
            &[COMPOSE_FILE, ENVIRONMENT_FILE]
        } else {
            &[COMPOSE_FILE]
        };
        if purposes.len() != required.len()
            || !required
                .iter()
                .all(|name| purposes.iter().any(|purpose| purpose == *name))
        {
            return None;
        }
        let seconds = item["limits"]["seconds"]
            .as_u64()
            .filter(|seconds| (1..=1800).contains(seconds))?;
        Some(Self {
            parameters,
            lifetime: Duration::from_secs(seconds),
            lease: serde_json::from_value(item["lease"].clone()).ok()?,
        })
    }
}

fn is_hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub(super) async fn run(
    state: State,
    client: &Client,
    authority: &Authority,
    (held, item): (Held, &Value),
) {
    let started = Instant::now();
    let end = match Work::read(item) {
        Some(work) => match apply(&state, (client, authority), (held, item), work).await {
            Ok(health) => End::Applied(health),
            Err(end) => end,
        },
        None => End::Refused("member_unknown"),
    };
    if let Some(report) = end.report(held, started.elapsed().as_secs()) {
        write(&state, |journal| journal.result(report));
    } else {
        write(&state, |journal| journal.settle(held, Utc::now()));
    }
}

async fn apply(
    state: &State,
    (client, authority): (&Client, &Authority),
    (held, item): (Held, &Value),
    work: Work,
) -> Result<Health, End> {
    let mut lease = Lease::new((authority, held), work.lease, work.lifetime);
    match lease.confirm(WorkState::Preparing).await {
        Ok(_) => (),
        Err(end @ End::Stopped(_)) => return Err(end),
        Err(_) => return Err(End::Lost),
    }
    let files = fetch(authority, held, &mut lease, &work.parameters).await?;
    let owner_deadline = Instant::now() + work.lifetime + TIMEOUT;
    let prepared = lease
        .while_held(WorkState::Preparing, client.stage_apply(item, files))
        .await?
        .map_err(End::Failed)?;
    if !write(state, |journal| journal.fence(held)) {
        return Err(End::Lost);
    }
    let ack = lease.confirm(WorkState::Running).await?;
    let stream = time::timeout_at(lease.end(), prepared.start(ack))
        .await
        .map_err(|_| End::Failed("orphan"))?
        .map_err(|_| End::Failed("orphan"))?;
    observe(stream, &mut lease, owner_deadline).await
}

async fn fetch(
    authority: &Authority,
    held: Held,
    lease: &mut Lease<'_>,
    parameters: &ApplicationApply,
) -> Result<ApplyFiles, End> {
    let compose = read_file(
        authority,
        held,
        lease,
        (COMPOSE_FILE, &parameters.file_sha256),
    )
    .await?;
    let environment = if let Some(hash) = &parameters.environment_sha256 {
        lease.confirm(WorkState::Preparing).await?;
        Some(read_file(authority, held, lease, (ENVIRONMENT_FILE, hash)).await?)
    } else {
        None
    };
    let files = ApplyFiles {
        compose,
        environment,
    };
    if !files.live() {
        return Err(End::Failed("secret_failed"));
    }
    Ok(files)
}

async fn read_file(
    authority: &Authority,
    held: Held,
    lease: &Lease<'_>,
    (purpose, hash): (&'static str, &str),
) -> Result<ReleasedSecret, End> {
    let call = authority.hub.apply_file(&authority.token, held, purpose);
    match time::timeout_at(lease.end(), call).await {
        Ok(Answer::Accepted(file)) if hex(&Sha256::digest(file.bytes())) == hash => Ok(file),
        Ok(Answer::Refused(code)) if relinquished(code) => Err(End::Lost),
        _ => Err(End::Failed("secret_failed")),
    }
}

async fn observe(
    mut stream: UnixStream,
    lease: &mut Lease<'_>,
    drain_by: Instant,
) -> Result<Health, End> {
    let result = result_of(&mut stream, lease).await;
    if let Err(end) = result {
        let _ = stream.shutdown().await;
        let drained = time::timeout_at(
            drain_by,
            tokio::io::copy(&mut stream, &mut tokio::io::sink()),
        )
        .await;
        if matches!(end, End::Stopped(_) | End::Expired) && !matches!(drained, Ok(Ok(_))) {
            return Err(End::Failed("orphan"));
        }
        return Err(end);
    }
    result
}

async fn result_of(stream: &mut UnixStream, lease: &mut Lease<'_>) -> Result<Health, End> {
    let mut phase = WorkState::Running;
    loop {
        let reply = lease
            .while_held(phase, applications::read(stream))
            .await?
            .map_err(|_| End::Failed("orphan"))?;
        match reply {
            Reply::CheckingHealth if phase == WorkState::Running => {
                phase = WorkState::Finishing;
                lease.confirm(phase).await?;
            }
            Reply::Applied { health } if phase == WorkState::Finishing => return Ok(health),
            other @ Reply::Failed { .. } => return Err(End::Failed(apply_failure(other))),
            _ => return Err(End::Failed("orphan")),
        }
    }
}

impl End {
    fn report(self, held: Held, seconds: u64) -> Option<WorkReport> {
        match self {
            Self::Lost => None,
            Self::Refused(cause) => Some(reports::terminal(
                held,
                WorkState::Refused,
                Some(cause),
                seconds,
            )),
            Self::Failed(cause) => Some(reports::terminal(
                held,
                WorkState::Failed,
                Some(cause),
                seconds,
            )),
            Self::Applied(health) => Some(reports::applied(held, &health, seconds)),
            Self::Stopped(reason) => Some(reports::stopped(held, reason, seconds)),
            Self::Expired => Some(reports::stopped(held, StopReason::Expired, seconds)),
        }
    }
}
