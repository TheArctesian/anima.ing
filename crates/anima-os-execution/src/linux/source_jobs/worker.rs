use super::{
    Error, Grant, Phase, Record, Result, advance, checkout, current, journal, snapshot, store,
    supervisor,
};
use crate::linux::{builds, command, workspace};
use anima_os_maintenance::{
    Error as AdmissionError, capacity::Resources, source_pipeline::Pipeline,
};
use serde_json::json;
use std::{num::NonZeroU64, time::Duration};
use uuid::Uuid;

mod checkout_workspace;
#[cfg(test)]
mod tests;

pub(super) fn limits(record: &Record) -> Result<(Resources, u64)> {
    let limits = record.job.limits;
    let storage = limits
        .disk_bytes
        .saturating_sub(20 * 1024_u64.pow(3))
        .min(4 * 1024_u64.pow(3));
    let supported = record.job.secrets.is_empty()
        || record.coordinated && daemon_core::source_jobs::private_checkout(&record.job);
    if !supported || storage < 1024_u64.pow(3) || limits.memory_bytes < 512 * 1024 * 1024 {
        return Err(Error::Refused(
            "This source supervisor needs authorized source, at least 512 MiB memory and 21 GiB total disk budget.",
        ));
    }
    Ok((
        Resources {
            memory_bytes: limits.memory_bytes.min(2 * 1024_u64.pow(3)),
            cpu_millis: 500,
            tasks: 128,
        },
        storage,
    ))
}

pub(in crate::linux) async fn run(id: Uuid) -> Result<()> {
    let pid = command::run(
        "systemctl",
        &[
            "show",
            &supervisor::unit(id),
            "--property=MainPID",
            "--value",
        ],
    )
    .await?;
    if pid.trim() != std::process::id().to_string() {
        return Err(Error::Refused(
            "Run source execution through its registered supervisor.",
        ));
    }
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let execution = execute(id);
    tokio::pin!(execution);
    let result = tokio::select! {
        result = &mut execution => result,
        _ = terminate.recv() => {
            journal(|| mark_stopping(id)).await?;
            // Let the bounded owner call settle before ExecStopPost proves termination.
            execution.await
        }
    };
    if let Err(error) = &result {
        journal(|| mark_failed(id, error)).await?;
    }
    result
}

fn mark_failed(id: Uuid, error: &Error) -> Result<()> {
    let _lock = store::lock()?;
    let mut record = current(id)?;
    if record.phase.executing() && record.phase != Phase::Cancelling {
        let failure = match record.phase {
            Phase::Starting => "controller_start_failed",
            Phase::Preparing => "workspace_prepare_failed",
            Phase::AwaitingCheckoutCredential => "secret_failed",
            Phase::CheckingOut => "source_checkout_failed",
            Phase::Capturing => "source_capture_failed",
            Phase::Building if matches!(error, Error::BuildOutOfMemory) => "limit_memory",
            Phase::Building => "source_build_failed",
            Phase::Finishing => "source_finish_failed",
            _ => "execution_failed",
        };
        record.phase = Phase::Cancelling;
        record.failure = Some(failure.into());
        store::save(&record)?;
    }
    Ok(())
}

fn mark_stopping(id: Uuid) -> Result<()> {
    let _lock = store::lock()?;
    let mut record = current(id)?;
    if record.phase.executing() {
        record.phase = Phase::Cancelling;
        record
            .failure
            .get_or_insert_with(|| "execution_stopped".into());
        record.result = None;
        store::save(&record)?;
    }
    Ok(())
}

async fn execute(id: Uuid) -> Result<()> {
    let pipeline = Pipeline::resume(id)?;
    let record = journal(|| advance(id, Phase::Preparing)).await?;
    if (record.workspace, record.build) != (pipeline.workspace(), pipeline.build()) {
        return Err(Error::Refused(
            "The source journal does not match pipeline admission.",
        ));
    }
    let (resources, storage) = limits(&record)?;
    checkout_workspace::start(pipeline, record.lease.grant, resources, storage).await?;
    checkout_source(id, daemon_core::source_jobs::private_checkout(&record.job)).await?;
    capture_source(id, pipeline).await?;
    let build = start_build(id, pipeline, resources).await?;
    wait(id, build).await
}

async fn start_build(id: Uuid, pipeline: Pipeline, resources: Resources) -> Result<Uuid> {
    let record = journal(|| advance(id, Phase::Building)).await?;
    let strategy = match record.job.parameters["strategy"].as_str() {
        Some("auto") => None,
        Some("dockerfile") => Some(builds::worker::Strategy::Dockerfile),
        Some("railpack") => Some(builds::worker::Strategy::Railpack),
        _ => return Err(Error::Refused("The source strategy is unsupported.")),
    };
    let source = workspace::context::Identity {
        workspace: record.workspace,
        generation: 1,
        context: record.context,
    };
    let mut choice = builds::Choice {
        supervisor: Some(pipeline),
        lease: Some(record.lease.grant),
        resources,
        strategy,
        seconds: NonZeroU64::new(record.job.limits.seconds)
            .ok_or(Error::Refused("Invalid source runtime budget."))?,
    };
    loop {
        let current = journal(|| snapshot(id)).await?;
        if current.phase != Phase::Building
            || !current.lease.matches(record.lease.grant)
            || (current.workspace, current.context, current.build)
                != (record.workspace, record.context, record.build)
        {
            return Err(Error::Refused(
                "Source-job execution is no longer authorized to build this source.",
            ));
        }
        choice.lease = Some(current.lease.grant);
        match builds::start(record.build, source, choice).await {
            Err(Error::Admission(
                AdmissionError::HostingPressure | AdmissionError::PressureUnavailable,
            )) if !builds::store::base(record.build).try_exists()? => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            result => return result.map(|_| record.build),
        }
    }
}

async fn capture_source(id: Uuid, pipeline: Pipeline) -> Result<()> {
    let record = journal(|| advance(id, Phase::Capturing)).await?;
    workspace::stop(record.workspace, 1).await?;
    let captured = capture_while_leased(
        pipeline,
        workspace::context::Identity {
            workspace: record.workspace,
            generation: 1,
            context: record.context,
        },
        record.lease.grant,
    )
    .await?;
    if captured["phase"] != "captured" {
        return Err(Error::Refused("The source context could not be captured."));
    }
    Ok(())
}

async fn capture_while_leased(
    pipeline: Pipeline,
    identity: workspace::context::Identity,
    grant: Grant,
) -> Result<serde_json::Value> {
    loop {
        let current = journal(|| snapshot(pipeline.execution())).await?;
        if current.phase != Phase::Capturing
            || !current.lease.matches(grant)
            || (current.workspace, current.context, current.build)
                != (identity.workspace, identity.context, pipeline.build())
        {
            return Err(Error::Refused(
                "Source-job execution is no longer authorized to capture this source.",
            ));
        }
        match workspace::context::capture(identity, Some(pipeline)).await {
            Err(Error::Admission(
                AdmissionError::Busy
                | AdmissionError::HostingPressure
                | AdmissionError::PressureUnavailable,
            )) => {
                // Both checks occur before capture creates a context or takes its snapshot.
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            result => return result,
        }
    }
}

async fn checkout_source(id: Uuid, private: bool) -> Result<()> {
    let (record, credential) = if private {
        let listener = checkout::Listener::bind(id)?;
        journal(|| advance(id, Phase::AwaitingCheckoutCredential)).await?;
        let (credential, mut connection) = listener.receive().await?;
        let Some(credential) = credential else {
            checkout::acknowledge(&mut connection).await;
            return Err(Error::Refused(
                "The private checkout credential was not delivered.",
            ));
        };
        let record = journal(|| advance(id, Phase::CheckingOut)).await?;
        checkout::acknowledge(&mut connection).await;
        (record, Some(credential))
    } else {
        (journal(|| advance(id, Phase::CheckingOut)).await?, None)
    };
    let source = serde_json::from_value(
        json!({"repository":record.job.repository,"commit":record.job.parameters["commit"]}),
    )?;
    workspace::checkout::checkout(record.workspace, 1, source, credential).await?;
    Ok(())
}

async fn wait(id: Uuid, build: Uuid) -> Result<()> {
    loop {
        let record = journal(|| snapshot(id)).await?;
        if record.phase != Phase::Building {
            return Err(Error::Refused("Source execution was stopped."));
        }
        let status = match builds::status(build).await {
            Err(Error::BuildAttemptBusy) => {
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
            result => result?,
        };
        if status["runtime"]["MainPID"] == "0"
            && matches!(
                status["runtime"]["ActiveState"].as_str(),
                Some("inactive" | "failed")
            )
        {
            let result = match builds::reconcile(build).await {
                Err(Error::BuildAttemptBusy | Error::Admission(AdmissionError::Busy)) => {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    continue;
                }
                result => result?,
            };
            if result["phase"] == "failed" && status["runtime"]["Result"] == "oom-kill" {
                return Err(Error::BuildOutOfMemory);
            }
            if result["phase"] != "verified" {
                return Err(Error::Refused(
                    "The source build did not produce a verified artifact.",
                ));
            }
            journal(|| finish(id, &result["result"])).await?;
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn finish(id: Uuid, result: &serde_json::Value) -> Result<()> {
    let _lock = store::lock()?;
    let mut record = current(id)?;
    if record.phase != Phase::Building {
        return Err(Error::Refused(
            "Source execution lost its lease before completion.",
        ));
    }
    record.result = Some(result.clone());
    record.phase = Phase::Finishing;
    store::save(&record)
}
