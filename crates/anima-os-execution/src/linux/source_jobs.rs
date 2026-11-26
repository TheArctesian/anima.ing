//! Root-owned source-job lifecycle; the hub never selects a command or path.
mod artifact;
pub(super) mod broker;
mod capacity;
mod checkout;
mod cleanup;
mod inventory;
mod registration;
mod retirement;
mod stopping;
mod store;
mod supervisor;
mod worker;
use super::{
    Error, Result,
    builds::lease::{Grant, Lease},
    work_policy,
};
use chrono::{DateTime, Utc};
pub(super) use cleanup::run as run_cleanup;
use daemon_core::{
    artifact::Receipt,
    wire::{WorkJob, WorkType},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub(super) use supervisor::{watch, watch_loop};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub(super) enum Request {
    #[serde(rename = "source-job-capacity")]
    Capacity,
    #[serde(rename = "source-job-release")]
    Release { execution: Uuid, confirmed: bool },
    #[serde(rename = "source-job-list")]
    List { device: Uuid, origin: String },
    #[serde(rename = "source-job-register")]
    Register {
        execution: Uuid,
        device: Uuid,
        origin: String,
        item: Value,
    },
    #[serde(rename = "source-job-start")]
    Start { execution: Uuid },
    #[serde(rename = "source-job-status")]
    Status { execution: Uuid },
    #[serde(rename = "source-job-renew")]
    Renew { execution: Uuid, lease: Grant },
    #[serde(rename = "source-job-cancel")]
    Cancel { execution: Uuid },
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Registered,
    Starting,
    Preparing,
    AwaitingCheckoutCredential,
    CheckingOut,
    Capturing,
    Building,
    Finishing,
    Cancelling,
    Ready,
    Failed,
    Expired,
    Cancelled,
    Releasing,
    Released,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u32,
    execution: Uuid,
    device: Uuid,
    origin: String,
    job: WorkJob,
    lease: Lease,
    registered_at: DateTime<Utc>,
    #[serde(default)]
    coordinated: bool,
    phase: Phase,
    workspace: Uuid,
    context: Uuid,
    build: Uuid,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release_receipt: Option<Receipt>,
}

impl Phase {
    fn executing(&self) -> bool {
        matches!(
            self,
            Self::Starting
                | Self::Preparing
                | Self::AwaitingCheckoutCredential
                | Self::CheckingOut
                | Self::Capturing
                | Self::Building
                | Self::Finishing
                | Self::Cancelling
        )
    }
}

impl Request {
    pub(super) async fn execute(self) -> Result<Value> {
        match self {
            Self::Capacity => capacity::observe().await,
            Self::Release {
                execution,
                confirmed,
            } => cleanup::release(execution, confirmed).await,
            Self::Start { execution } => supervisor::start(execution).await,
            Self::Cancel { execution } => stopping::cancel(execution).await,
            request => request.metadata(),
        }
    }

    fn metadata(self) -> Result<Value> {
        let _lock = store::lock()?;
        match self {
            Self::List { device, origin } => inventory::list(device, &origin, false),
            Self::Register {
                execution,
                device,
                origin,
                item,
            } => registration::register(execution, (device, origin), &item, false),
            Self::Status { execution } => Ok(serde_json::to_value(current(execution)?)?),
            Self::Renew { execution, lease } => {
                let mut record = current(execution)?;
                if record.phase != Phase::Registered
                    && (!record.phase.executing() || record.phase == Phase::Cancelling)
                {
                    return Err(Error::Refused("This source job is no longer renewable."));
                }
                work_policy::check_item(&serde_json::to_value(&record.job)?)?;
                record.lease = record.lease.extend(lease)?;
                store::save(&record)?;
                if record.phase == Phase::Building {
                    super::builds::lease::renew_supervised(record.build, execution, lease)?;
                }
                Ok(serde_json::to_value(record)?)
            }
            Self::Capacity | Self::Start { .. } | Self::Cancel { .. } | Self::Release { .. } => {
                Err(Error::Refused(
                    "This source-job request requires runtime dispatch.",
                ))
            }
        }
    }
}

pub(super) fn require_build_lease(
    pipeline: anima_os_maintenance::source_pipeline::Pipeline,
    source: super::workspace::context::Identity,
    grant: Grant,
) -> Result<Lease> {
    let denied = "The source job no longer owns this build.";
    let record = require_pipeline_lease(pipeline, grant, &Phase::Building, denied)?;
    if source.generation != 1
        || source.workspace != pipeline.workspace()
        || record.context != source.context
    {
        return Err(Error::Refused(denied));
    }
    Ok(record.lease)
}

pub(super) fn require_checkout_lease(
    pipeline: anima_os_maintenance::source_pipeline::Pipeline,
    grant: Grant,
) -> Result<()> {
    require_pipeline_lease(
        pipeline,
        grant,
        &Phase::Preparing,
        "The source job no longer owns this checkout workspace.",
    )
    .map(drop)
}

fn require_pipeline_lease(
    pipeline: anima_os_maintenance::source_pipeline::Pipeline,
    grant: Grant,
    phase: &Phase,
    denied: &'static str,
) -> Result<Record> {
    let record = store::read(pipeline.execution())?.ok_or(Error::Refused(denied))?;
    if &record.phase != phase
        || !record.lease.matches(grant)
        || record.lease.expired()
        || (record.workspace, record.build) != (pipeline.workspace(), pipeline.build())
    {
        return Err(Error::Refused(denied));
    }
    Ok(record)
}

fn current(execution: Uuid) -> Result<Record> {
    let mut record = store::read(execution)?
        .ok_or(Error::Refused("Register this source job before using it."))?;
    if record.lease.expired() {
        if record.phase == Phase::Registered {
            record.phase = Phase::Expired;
            store::save(&record)?;
        } else if record.phase.executing() && record.phase != Phase::Cancelling {
            record.phase = Phase::Cancelling;
            record.failure = Some("lease_expired".into());
            record.result = None;
            store::save(&record)?;
        }
    }
    Ok(record)
}

fn snapshot(execution: Uuid) -> Result<Record> {
    let _lock = store::lock()?;
    current(execution)
}

async fn journal<T>(operation: impl Fn() -> Result<T>) -> Result<T> {
    loop {
        match operation() {
            Err(Error::SourceJournalBusy) => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            result => return result,
        }
    }
}

fn advance(execution: Uuid, phase: Phase) -> Result<Record> {
    let _lock = store::lock()?;
    let mut record = current(execution)?;
    if !record.phase.executing() || record.phase == Phase::Cancelling {
        return Err(Error::Refused(
            "Source-job execution is no longer authorized.",
        ));
    }
    record.phase = phase;
    store::save(&record)?;
    Ok(record)
}

pub(super) async fn dispatch(operation: &str, identity: &str) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused(
            "Source supervision requires the root owner.",
        ));
    }
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o077));
    let id = identity
        .parse()
        .map_err(|_| Error::Refused("Invalid source execution identity."))?;
    if operation == "source-job-worker" {
        worker::run(id).await
    } else {
        stopping::finish(id).await
    }
}

pub(super) async fn recover() -> Result<()> {
    watch().await?;
    let ids = {
        let _lock = store::lock()?;
        store::ids()?
    };
    for id in ids {
        if snapshot(id)?.phase == Phase::Releasing {
            cleanup::release(id, true).await?;
        }
    }
    Ok(())
}

fn authorized(execution: Uuid, device: Uuid, origin: &str) -> Result<Record> {
    let _lock = store::lock()?;
    let record = store::read(execution)?
        .ok_or(Error::Refused("Register this source job before using it."))?;
    if record.device != device || record.origin != origin || !record.coordinated {
        return Err(Error::Refused(
            "This execution is not coordinated by this enrollment.",
        ));
    }
    Ok(record)
}
