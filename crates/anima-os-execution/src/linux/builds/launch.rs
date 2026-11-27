mod admission;
use admission::LockedAttempt;

use super::super::{
    Error, Result, command,
    workspace::{context, record},
};
use super::{lease::Lease, sandbox::Sandbox, store, worker};
use crate::linux::source_jobs;
use anima_os_maintenance::{capacity::Resources, source_pipeline::Pipeline};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct Choice {
    pub supervisor: Option<Pipeline>,
    pub seconds: std::num::NonZeroU64,
    pub lease: Option<super::lease::Grant>,
    pub resources: Resources,
    pub strategy: Option<worker::Strategy>,
}

impl Choice {
    fn resolve(self, source: &Path) -> Result<worker::Strategy> {
        let dockerfile = source.join("Dockerfile").is_file();
        let strategy = self.strategy.unwrap_or(if dockerfile {
            worker::Strategy::Dockerfile
        } else {
            worker::Strategy::Railpack
        });
        if strategy == worker::Strategy::Dockerfile && !dockerfile {
            return Err(Error::Refused(
                "The selected Dockerfile strategy needs a Dockerfile.",
            ));
        }
        Ok(strategy)
    }
}

pub async fn start(id: Uuid, source: context::Identity, choice: Choice) -> Result<Value> {
    if let Some(pipeline) = choice.supervisor
        && (choice.lease.is_none()
            || pipeline.build() != id
            || pipeline.workspace() != source.workspace
            || source.generation != 1)
    {
        return Err(Error::Refused(
            "The build does not match its source pipeline admission.",
        ));
    }
    let resources = choice.resources;
    require_resources(resources)?;
    let locks = LockedAttempt::acquire(id, choice.supervisor)?;
    let allocation = record::allocation()?;
    let base = store::base(id);
    if base.try_exists()? {
        return previous(id, source, choice);
    }
    locks.require_open_work()?;
    if choice.lease.is_some() {
        command::run(
            "systemctl",
            &["is-active", "--quiet", "anima-build-leases.service"],
        )
        .await?;
    }
    require_capacity(&allocation)?;
    let (sandbox, toolchain) = Sandbox::load()?;
    let frontend = toolchain["railpackFrontend"]
        .as_str()
        .ok_or(Error::Refused("The Railpack frontend is missing."))?
        .to_owned();
    let user = nix::unistd::User::from_name("anima-builder")
        .map_err(std::io::Error::from)?
        .ok_or(Error::Refused("The isolated builder account is missing."))?;
    let mut attempt = begin(id, source, choice, toolchain)?;
    let pinned = context::pin(&allocation, source, id)?;
    let context = pinned.directory;
    let strategy = choice.resolve(&context)?;
    attempt.source_digest = Some(pinned.digest);
    attempt.source = pinned.checkout;
    attempt.strategy = Some(strategy);
    store::save(&attempt)?;
    // The saved attempt accounts for disk space from here through release.
    drop(allocation);
    let locks = locks.reserve(&attempt, choice.supervisor).await?;
    prepare_work(&base, strategy, frontend, &user).await?;
    let _locks = locks.reserve(&attempt, choice.supervisor).await?;
    activate(attempt, &sandbox, &context, choice.supervisor).await
}

async fn activate(
    mut attempt: store::Attempt,
    sandbox: &Sandbox,
    context: &Path,
    supervisor: Option<Pipeline>,
) -> Result<Value> {
    if let Some(pipeline) = supervisor {
        attempt.lease = Some(supervised_lease(&attempt, pipeline)?);
    }
    if attempt
        .lease
        .as_ref()
        .is_some_and(super::lease::Lease::expired)
    {
        return Err(Error::Refused(
            "The build lease expired during preparation.",
        ));
    }
    if let Some(source) = &attempt.source {
        source.authorize()?;
    }
    let mut arguments = sandbox.arguments(&attempt, context)?;
    if attempt.lease.is_some() {
        let dependencies = attempt.supervisor.map_or_else(
            || "anima-build-leases.service".to_owned(),
            |id| format!("anima-build-leases.service anima-source-job-{id}.service"),
        );
        arguments.splice(
            0..0,
            [
                format!("--property=BindsTo={dependencies}"),
                format!("--property=After={dependencies}"),
            ],
        );
    }
    attempt.phase = "launching".into();
    store::save(&attempt)?;
    let arguments: Vec<_> = arguments.iter().map(String::as_str).collect();
    match command::run("systemd-run", &arguments).await {
        Ok(_) => attempt.phase = "running".into(),
        Err(_) => attempt.phase = "unknown".into(),
    }
    store::save(&attempt)?;
    Ok(serde_json::to_value(attempt)?)
}

fn supervised_lease(attempt: &store::Attempt, pipeline: Pipeline) -> Result<Lease> {
    let grant = attempt
        .lease
        .as_ref()
        .ok_or(Error::Refused("The supervised build lease is missing."))?
        .grant;
    let source = context::Identity {
        workspace: attempt.workspace,
        generation: attempt.generation,
        context: attempt.context,
    };
    source_jobs::require_build_lease(pipeline, source, grant)
}

fn begin(
    id: Uuid,
    source: context::Identity,
    choice: Choice,
    toolchain: Value,
) -> Result<store::Attempt> {
    let context::Identity {
        workspace,
        generation,
        context: source_id,
    } = source;
    let base = store::base(id);
    let lease = choice
        .lease
        .map(|grant| match choice.supervisor {
            Some(pipeline) => source_jobs::require_build_lease(pipeline, source, grant),
            None => Lease::new(grant),
        })
        .transpose()?;
    fs::DirBuilder::new().mode(0o700).create(&base)?;
    let attempt = store::Attempt {
        schema: 1,
        supervisor: choice.supervisor.map(Pipeline::execution),
        lease,
        id,
        context: source_id,
        workspace,
        generation,
        resources: choice.resources,
        seconds: choice.seconds.min(store::default_seconds()),
        phase: "preparing".into(),
        source_digest: None,
        source: None,
        strategy: None,
        requested_strategy: choice.strategy,
        result: None,
        toolchain: Some(toolchain),
    };
    store::save(&attempt)?;
    Ok(attempt)
}

fn require_resources(resources: Resources) -> Result<()> {
    if resources.memory_bytes < 512 * 1024 * 1024
        || resources.cpu_millis < 250
        || resources.tasks < 64
    {
        return Err(Error::Refused(
            "The builder needs at least 512 MiB, a quarter CPU and 64 tasks.",
        ));
    }
    Ok(())
}

fn previous(id: Uuid, source: context::Identity, choice: Choice) -> Result<Value> {
    let context::Identity {
        workspace,
        generation,
        context: source_id,
    } = source;
    let existing = store::required(id)?;
    let same_lease = match (&existing.lease, choice.lease) {
        (None, None) => true,
        (Some(lease), Some(grant)) => lease.matches(grant),
        _ => false,
    };
    if existing.seconds != choice.seconds.min(store::default_seconds())
        || existing.supervisor != choice.supervisor.map(Pipeline::execution)
        || !same_lease
        || existing.workspace != workspace
        || existing.generation != generation
        || existing.context != source_id
        || existing.resources != choice.resources
        || existing.requested_strategy != choice.strategy
    {
        return Err(Error::Refused(
            "The build identity already belongs to a different request.",
        ));
    }
    Ok(serde_json::to_value(existing)?)
}

fn require_capacity(allocation: &record::Allocation) -> Result<()> {
    if record::free_storage(allocation)? >= store::QUOTA {
        return Ok(());
    }
    Err(Error::Refused(
        "Hosting reserves the remaining build storage. Choose another node.",
    ))
}

async fn prepare_work(
    base: &Path,
    strategy: worker::Strategy,
    frontend: String,
    user: &nix::unistd::User,
) -> Result<()> {
    let work = base.join("work");
    command::run("btrfs", &["subvolume", "create", text(&work)?]).await?;
    command::run(
        "btrfs",
        &["qgroup", "limit", &store::QUOTA.to_string(), text(&work)?],
    )
    .await?;
    nix::unistd::chown(&work, Some(user.uid), Some(user.gid)).map_err(std::io::Error::from)?;
    let request = base.join("request.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&request)?;
    file.write_all(&serde_json::to_vec(&worker::Request {
        schema: 1,
        strategy,
        frontend,
    })?)?;
    fs::set_permissions(
        &request,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o444),
    )?;
    file.sync_all()?;
    Ok(())
}

fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or(Error::Refused("The build path is invalid."))
}
