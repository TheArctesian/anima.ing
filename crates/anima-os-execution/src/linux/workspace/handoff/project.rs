//! Export the stopped project without opening the workspace's provider home.
pub(super) mod archive;
pub(super) mod digest;

use super::{
    authorize,
    withdrawal::{self, Side},
};
use crate::linux::{
    Error, Result,
    workspace::{
        cleanup,
        record::{self, Phase},
    },
};
use agent_threads::managed::workspace::{
    Capacity,
    handoff::{ARCHIVE_OVERHEAD, MAX_ENTRIES, Receipt, Source},
};
use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub(super) const DEADLINE: Duration = Duration::from_mins(20);
struct Project {
    source: Source,
    quota: u64,
    _locks: (File, File),
}

pub(in crate::linux) async fn describe(source: Source) -> Result<Value> {
    let project = open(source).await?;
    let limit = project.quota.saturating_add(ARCHIVE_OVERHEAD);
    let receipt = run(project, limit, io::sink()).await?;
    Ok(serde_json::to_value(receipt)?)
}

pub(in crate::linux) async fn stream(receipt: Receipt) -> Result<()> {
    if !receipt.fits(Capacity::MAXIMUM.storage_bytes) {
        return Err(Error::Refused("The project checkpoint receipt is invalid."));
    }
    let project = open(receipt.source).await?;
    if !receipt.fits(project.quota) {
        return Err(Error::Refused("The project checkpoint receipt is invalid."));
    }
    let output = io::BufWriter::with_capacity(128 * 1024, io::stdout());
    let observed = run(project, receipt.bytes, output).await?;
    if observed != receipt {
        return Err(Error::Refused(
            "The streamed project differs from its checkpoint receipt.",
        ));
    }
    Ok(())
}

async fn open(source: Source) -> Result<Project> {
    if !source.fits() {
        return Err(Error::Refused("The project checkpoint source is invalid."));
    }
    let checkpoint = checkpoint_lock()?;
    let workspace = record::lock(source.workspace)?;
    let state = record::required(source.workspace)?;
    authorize(&state, source.generation, source.principal)?;
    withdrawal::require_open(source, Side::Source)?;
    if state.phase != Phase::Stopped || state.handoff != Some(source.binding) {
        return Err(Error::Refused(
            "Fence this project for the exact transfer before exporting it.",
        ));
    }
    tokio::time::timeout(
        Duration::from_secs(10),
        cleanup::absent_runtime(source.workspace),
    )
    .await
    .map_err(|_| Error::Refused("The source runtime could not be checked."))??;
    anima_os_maintenance::pressure::require_clear()?;
    Ok(Project {
        source,
        quota: state.storage_bytes,
        _locks: (checkpoint, workspace),
    })
}

pub(super) fn checkpoint_lock() -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/persist/anima-workspaces/checkpoint.lock")?;
    file.try_lock()
        .map_err(|_| Error::Refused("Another project checkpoint is active."))?;
    Ok(file)
}

async fn run(
    project: Project,
    bytes: u64,
    output: impl io::Write + Send + 'static,
) -> Result<Receipt> {
    let deadline = Instant::now() + DEADLINE;
    let capture = tokio::task::spawn_blocking(move || {
        lower_priority()?;
        let Project {
            source,
            quota,
            _locks,
        } = project;
        let home = record::home(source.workspace);
        let dir = archive::open_project(&home)?;
        let limits = archive::Limits {
            quota,
            bytes,
            deadline,
        };
        let (digest, bytes, entries) = archive::write(&dir, output, limits)?;
        Ok(Receipt {
            schema: 1,
            source,
            digest,
            bytes,
            entries,
        })
    });
    tokio::time::timeout(DEADLINE, capture)
        .await
        .map_err(|_| Error::Refused("The project checkpoint exceeded its deadline."))?
        .map_err(|_| Error::Refused("The project checkpoint did not complete."))?
}

pub(super) fn lower_priority() -> Result<()> {
    let thread = nix::unistd::gettid().to_string();
    for (program, arguments) in [
        ("ionice", ["-c", "3", "-p", thread.as_str()]),
        ("renice", ["--priority", "19", "--pid", thread.as_str()]),
    ] {
        let status = Command::new(format!("/run/current-system/sw/bin/{program}"))
            .env_clear()
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(Error::Refused(
                "Project checkpoint priority could not be set.",
            ));
        }
    }
    Ok(())
}
