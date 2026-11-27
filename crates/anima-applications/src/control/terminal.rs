//! Resolve an exact, locally managed application container for administration.
mod attach;
mod engine;
mod inspection;
mod lifetime;

use std::{
    io,
    os::unix::net::UnixDatagram,
    path::Path,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    EngineKind, Settings,
    execution::{Fault, files::Store, version},
    final_form::image_by_digest,
    forms::{is_engine_name, is_lower_hex},
    plan::Project,
    startup::{self, Files, Ready},
};

use super::{apply, container};
use inspection::Inspection;

const LIMIT: Duration = Duration::from_secs(10);
const FALLBACK_USER: &str = "65534:65534";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Local administrator authorization is required.")]
    Owner,
    #[error("Choose a running, locally managed application container.")]
    Target,
    #[error("This application is protected.")]
    Protected,
    #[error("The container differs from the installed application.")]
    Changed,
    #[error("The container shares host privileges, devices or mounts.")]
    Isolation,
    #[error("The local application terminal could not continue ({0}).")]
    Terminal(io::ErrorKind),
    #[error(transparent)]
    Startup(#[from] startup::Error),
    #[error(transparent)]
    Execution(#[from] Fault),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Terminal(error.kind())
    }
}

impl From<rustix::io::Errno> for Error {
    fn from(error: rustix::io::Errno) -> Self {
        io::Error::from(error).into()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    container: String,
    name: String,
    application: String,
    service: String,
    user: String,
}

/// Inspect local administrator targets using the Hosting authority and current app record.
///
/// # Errors
/// The local caller lacks root authorization or the exact running instance is not eligible.
pub fn target(files: &Files<'_>, id: &str) -> Result<Target, Error> {
    let ready = local(files, id)?;
    let store = Store::open(Path::new(&ready.paths().root))?;
    inspect(files, &ready, &store, id)
}

fn local(files: &Files<'_>, id: &str) -> Result<Ready, Error> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Owner);
    }
    if !is_lower_hex(id, 64) {
        return Err(Error::Target);
    }
    let ready = startup::check(files)?;
    if ready.engine() != EngineKind::System {
        return Err(Error::Target);
    }
    Ok(ready)
}

fn inspect(files: &Files<'_>, ready: &Ready, store: &Store<'_>, id: &str) -> Result<Target, Error> {
    let paths = ready.paths();
    let deadline = Instant::now() + LIMIT;
    let seen = inspection::read(paths, id, deadline)?;
    let (application, name) = authorize(ready.settings(), &seen, id)?;
    let folders = paths.application(&application);
    store.absent(&folders.dot_env())?;
    version::require_settled(store, &folders)?;
    let records = apply::records::read(store, paths, &application)?;
    let installed = records
        .installed(&application)
        .filter(|record| record.has_files)
        .ok_or(Error::Changed)?;
    let service = seen.service.as_deref().ok_or(Error::Changed)?;
    let current = installed
        .current
        .iter()
        .find(|image| image.service == service)
        .ok_or(Error::Changed)?;
    if seen.folder.as_deref() != Some(&folders.files())
        || seen.final_form.as_deref() != Some(&folders.final_form())
        || seen.image != image_by_digest(&current.image, &current.digest)
    {
        return Err(Error::Changed);
    }
    let project = Project {
        name: seen.project.as_deref().ok_or(Error::Changed)?,
        folder: &folders.files(),
        files: &[folders.final_form()],
    };
    let hashes =
        apply::observe::service_hashes(paths, &project, deadline)?.ok_or(Error::Changed)?;
    if hashes
        .get(service)
        .is_none_or(|hash| Some(hash) != seen.config_hash.as_ref())
    {
        return Err(Error::Changed);
    }
    seen.check_mounts(store, paths, (&folders.binds(), deadline))?;
    if !startup::settings_current(files.settings, ready.settings()) {
        return Err(Error::Changed);
    }

    Ok(Target {
        container: seen.id,
        name,
        application,
        service: service.to_owned(),
        user: terminal_user(&seen.user).to_owned(),
    })
}

/// Open a local terminal for the resolved application under its declared non-root account.
///
/// # Errors
/// Authorization fails, the local terminal is unavailable, or the engine fails to attach.
pub fn open(files: &Files<'_>, id: &str) -> Result<(), Error> {
    let ready = local(files, id)?;
    let mut terminal = attach::Terminal::open()?;
    let paths = ready.paths();
    let store = Store::open(Path::new(&paths.root))?;
    let target = inspect(files, &ready, &store, id)?;
    let engine = engine::Engine::new(paths);
    let dimensions = terminal.dimensions()?;
    let nonce = lifetime::nonce()?;
    let instance = engine.create(&target, &nonce)?;
    let socket = engine.start(&instance, dimensions)?;
    let owner = lifetime::Process::verify(&engine, &instance, &target, &nonce)?;
    let mut watchdog = owner.watch()?;
    drop(store);
    terminal.header(&target)?;
    record(&target, "opened");
    let result = terminal.relay(socket, (&engine, &instance));
    let cleanup = owner.end();
    let _ = watchdog.try_wait();
    record(
        &target,
        if cleanup.is_ok() {
            "closed"
        } else {
            "exit_unconfirmed"
        },
    );
    result.and(cleanup)
}

fn record(target: &Target, phase: &str) {
    let Ok(log) = UnixDatagram::unbound() else {
        return;
    };
    if log.set_nonblocking(true).is_ok() {
        let message = format!(
            "<86>anima-applications: application_terminal phase={phase} container={} application={} account={}",
            target.container, target.application, target.user
        );
        let _ = log.send_to(message.as_bytes(), "/dev/log");
    }
}

/// Watch inherited pidfds for the exact shell and its local relay.
///
/// # Errors
/// A process identity or the inherited watch channel is invalid.
pub fn watch_relay(parent: i32) -> Result<(), Error> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Owner);
    }
    let parent = (parent > 1)
        .then(|| rustix::process::Pid::from_raw(parent))
        .flatten()
        .ok_or(Error::Terminal(io::ErrorKind::InvalidInput))?;
    lifetime::watch_relay(parent)
}

fn authorize(settings: &Settings, seen: &Inspection, id: &str) -> Result<(String, String), Error> {
    if seen.id != id || !seen.running || seen.workspace {
        return Err(Error::Target);
    }
    let project = seen.project.as_deref().ok_or(Error::Target)?;
    let application = container::application(project).ok_or(Error::Target)?;
    let name = seen
        .name
        .strip_prefix('/')
        .filter(|name| is_engine_name(name))
        .ok_or(Error::Target)?;
    if settings.protects(project) || settings.protects(name) {
        return Err(Error::Protected);
    }
    if !seen.isolated() {
        return Err(Error::Isolation);
    }
    Ok((application, name.to_owned()))
}

fn terminal_user(user: &str) -> &str {
    let non_root = |id: &str| {
        !id.starts_with('0')
            && id.bytes().all(|byte| byte.is_ascii_digit())
            && id.parse::<u32>().is_ok_and(|value| value < u32::MAX)
    };
    if user
        .split_once(':')
        .is_some_and(|(uid, gid)| non_root(uid) && non_root(gid))
    {
        user
    } else {
        FALLBACK_USER
    }
}
