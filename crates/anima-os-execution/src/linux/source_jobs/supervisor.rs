use super::{Error, Phase, Result, current, snapshot, stopping, store, work_policy, worker};
use crate::linux::command;
use anima_os_maintenance::{
    Admission,
    source_pipeline::{self, Pipeline},
};
use serde_json::Value;
use uuid::Uuid;

pub(super) fn unit(id: Uuid) -> String {
    format!("anima-source-job-{id}.service")
}

pub(super) async fn start(id: Uuid) -> Result<Value> {
    command::run(
        "systemctl",
        &["is-active", "--quiet", "anima-build-leases.service"],
    )
    .await?;
    let record = {
        let admission = Admission::acquire()?;
        let _lock = store::lock()?;
        let mut record = current(id)?;
        if record.phase != Phase::Registered {
            return Ok(serde_json::to_value(record)?);
        }
        worker::limits(&record)?;
        work_policy::check_item(&serde_json::to_value(&record.job)?)?;
        for other in store::ids()? {
            if other != id && current(other)?.phase.executing() {
                return Err(Error::Refused("This node already supervises a source job."));
            }
        }
        Pipeline::accept(&admission, (id, record.workspace, record.build))?;
        record.phase = Phase::Starting;
        store::save(&record)?;
        record
    };
    let arguments = arguments(id, record.job.limits.seconds)?;
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    // An uncertain dispatch retains its Starting intent; retry never starts another unit.
    command::run("systemd-run", &borrowed).await?;
    Ok(serde_json::to_value(snapshot(id)?)?)
}

fn arguments(id: Uuid, seconds: u64) -> Result<Vec<String>> {
    let executable = std::env::current_exe()?;
    let executable = executable
        .to_str()
        .filter(|path| {
            path.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        })
        .ok_or(Error::Refused(
            "Source supervisor executable path is invalid.",
        ))?;
    let mut args = vec![
        format!("--unit={}", unit(id)),
        "--service-type=exec".into(),
        "--quiet".into(),
    ];
    for property in [
        "User=root".into(), "UMask=0077".into(), "Slice=anima-agents.slice".into(),
        "BindsTo=anima-build-leases.service".into(), "After=anima-build-leases.service".into(),
        format!("RuntimeMaxSec={}", seconds.min(3900)), "TimeoutStopSec=180".into(),
        "KillMode=mixed".into(), "MemoryMax=256M".into(), "MemorySwapMax=0".into(),
        "TasksMax=64".into(), "CPUQuota=25%".into(), "CPUWeight=10".into(), "IOWeight=10".into(),
        "Nice=19".into(), "IOSchedulingClass=idle".into(), "NoNewPrivileges=true".into(),
        "CapabilityBoundingSet=CAP_SYS_ADMIN CAP_CHOWN CAP_DAC_OVERRIDE".into(),
        "ProtectSystem=strict".into(), "ProtectHome=true".into(), "PrivateTmp=true".into(),
        "ProtectKernelTunables=true".into(), "ProtectKernelModules=true".into(),
        "ProtectControlGroups=true".into(), "RestrictAddressFamilies=AF_UNIX".into(),
        "ReadWritePaths=/persist/anima-source-jobs /persist/anima-work-policy /persist/anima-workspaces /persist/anima-builds /persist/anima-admission /run/anima-os".into(),
        "StandardOutput=null".into(), "StandardError=null".into(),
        format!("ExecStopPost={executable} source-job-finish {id}"),
    ] { args.push(format!("--property={property}")); }
    args.extend([
        "--".into(),
        executable.into(),
        "source-job-worker".into(),
        id.to_string(),
    ]);
    Ok(args)
}

pub(in crate::linux) async fn watch() -> Result<()> {
    reap_terminal_admission()?;
    let ids = {
        let _lock = store::lock()?;
        store::ids()?
    };
    for id in ids {
        if snapshot(id)?.phase == Phase::Cancelling && stopping::stop(id).await.is_err() {
            eprintln!("Source-job termination awaits runtime reconciliation.");
        }
    }
    Ok(())
}

fn reap_terminal_admission() -> Result<()> {
    let Some(pipeline) = source_pipeline::active()? else {
        return Ok(());
    };
    let Ok(admission) = Admission::cleanup() else {
        return Ok(());
    };
    let _lock = store::lock()?;
    let id = pipeline.execution();
    if !current(id)?.phase.executing() {
        source_pipeline::retire(&admission, id)?;
    }
    Ok(())
}

pub(in crate::linux) async fn watch_loop() -> Result<()> {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(5));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        match watch().await {
            Ok(()) | Err(Error::SourceJournalBusy) => (),
            Err(error) => return Err(error),
        }
    }
}
