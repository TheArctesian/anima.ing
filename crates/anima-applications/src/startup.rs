//! Gather Linux startup facts and apply the hosting owner's trust policy.
mod configuration;
mod paths;
mod versions;

use std::{io, path::Path};

use nix::unistd::{Uid, User, getgrouplist};
use std::{ffi::CString, os::unix::fs::MetadataExt as _};

use crate::{
    EngineKind, Paths,
    execution::{Executor, Fault},
    settings::{Managed, Settings, SettingsFault},
    start::{self, StartFacts, StartRefusal, Version},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("startup could not read local state ({0})")]
    Io(io::ErrorKind),
    #[error("a startup path cannot be trusted")]
    UntrustedPath,
    #[error("the configured daemon user could not be resolved")]
    DaemonUser,
    #[error("the Docker configuration is invalid")]
    Configuration,
    #[error("installed AnimaOS hosting must use the system Docker socket at /run/docker.sock")]
    InstalledSocket,
    #[error("the startup version probe did not return a supported version")]
    Version,
    #[error(transparent)]
    Settings(#[from] SettingsFault),
    #[error(transparent)]
    Refused(#[from] StartRefusal),
    #[error(transparent)]
    Execution(#[from] Fault),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

impl From<rustix::io::Errno> for Error {
    fn from(error: rustix::io::Errno) -> Self {
        io::Error::from(error).into()
    }
}

/// Local paths chosen by the administrator or installed by the operating system.
pub struct Files<'a> {
    pub settings: &'a Path,
    pub configuration: &'a Path,
    pub applications: &'a Path,
}

pub struct Ready {
    paths: Paths,
    settings: Settings,
    engine: EngineKind,
    daemon: u32,
}

impl Ready {
    #[must_use]
    pub const fn engine(&self) -> EngineKind {
        self.engine
    }

    #[must_use]
    pub const fn accepted_daemon(&self) -> u32 {
        self.daemon
    }

    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    #[must_use]
    pub const fn paths(&self) -> &Paths {
        &self.paths
    }

    /// # Errors
    /// The socket changed, or the local application store is inaccessible or occupied.
    pub fn executor(&self) -> Result<Executor<'_>, Fault> {
        Executor::open(&self.paths)
    }
}

/// Check startup authority before any probe, then check the installed engine versions.
///
/// # Errors
/// Untrusted settings or paths, an impermissible engine, or unavailable startup probes.
pub fn check(files: &Files<'_>) -> Result<Ready, Error> {
    let owner = rustix::process::geteuid().as_raw();
    let settings = configuration::settings(files.settings)?;
    let managed = settings.managed.as_ref().ok_or(StartRefusal::Off)?;
    let (paths, facts) = observed(files, managed, owner)?;
    if paths.root == anima_os_maintenance::cutover::APPLICATION_ROOT
        && paths.socket != anima_os_maintenance::hosting::SYSTEM_SOCKET
    {
        return Err(Error::InstalledSocket);
    }
    let engine = start::authority(&settings, &facts)?;
    let (engine_version, compose_version) = versions::probe(&paths)?;
    start::versions(engine_version, compose_version)?;
    let daemon = managed.accepted_user;
    Ok(Ready {
        paths,
        settings,
        engine,
        daemon,
    })
}

fn observed(
    files: &Files<'_>,
    managed: &Managed,
    owner: u32,
) -> Result<(Paths, StartFacts), Error> {
    let socket = paths::resolve(Path::new(&managed.socket), owner, paths::Kind::Socket)?;
    let docker = paths::resolve(Path::new(&managed.docker), owner, paths::Kind::Program)?;
    let compose = paths::resolve(Path::new(&managed.compose), owner, paths::Kind::Program)?;
    let configuration = paths::resolve(
        files.configuration,
        owner,
        paths::Kind::ConfigurationDirectory,
    )?;
    let root = paths::resolve(files.applications, owner, paths::Kind::PrivateDirectory)?;
    let (members, logins) = configuration::docker(&configuration.real, owner)?;
    let can_connect =
        managed.engine == EngineKind::System && daemon_can_connect(&socket, managed.accepted_user)?;
    let facts = StartFacts {
        running_as: owner,
        administrators: Vec::new(),
        socket_is_a_socket: true,
        socket_owner: socket.metadata.uid(),
        accepted_user_can_connect: can_connect,
        above_the_socket: socket.entries,
        programs: docker.entries.into_iter().chain(compose.entries).collect(),
        configuration: paths::entry(&configuration.metadata),
        configuration_members: members,
        logins,
        // Authority is checked here. Versions are checked separately after probing.
        engine: Version(0, 0, 0),
        compose: Version(0, 0, 0),
    };
    Ok((
        Paths {
            docker: paths::text(docker.real)?,
            compose: paths::text(compose.real)?,
            socket: paths::text(socket.real)?,
            configuration: paths::text(configuration.real)?,
            root: paths::text(root.real)?,
        },
        facts,
    ))
}

fn daemon_can_connect(socket: &paths::Resolved, uid: u32) -> Result<bool, Error> {
    let user = User::from_uid(Uid::from_raw(uid))
        .map_err(|_| Error::DaemonUser)?
        .ok_or(Error::DaemonUser)?;
    let name = CString::new(user.name).map_err(|_| Error::DaemonUser)?;
    let groups = getgrouplist(&name, user.gid).map_err(|_| Error::DaemonUser)?;
    let metadata = &socket.metadata;
    if uid == 0 || metadata.mode() & 0o002 != 0 {
        return Ok(true);
    }
    if metadata.mode() & 0o020 == 0 {
        return Ok(false);
    }
    // Linux exposes the ACL mask as the group bits. When it permits write, treat an
    // extended ACL as a possible grant rather than assuming the daemon cannot connect.
    Ok(
        groups.iter().any(|group| group.as_raw() == metadata.gid())
            || paths::has_acl(&socket.real)?,
    )
}

pub(crate) fn socket_directory(path: &Path) -> Result<(), Error> {
    let owner = rustix::process::geteuid().as_raw();
    let resolved = paths::resolve(path, owner, paths::Kind::ConfigurationDirectory)?;
    if resolved.real != path {
        return Err(Error::UntrustedPath);
    }
    Ok(())
}

pub(crate) fn settings_current(path: &Path, expected: &Settings) -> bool {
    configuration::settings(path).is_ok_and(|current| current == *expected)
}

pub(crate) fn protected_program(path: &Path) -> Result<std::path::PathBuf, Error> {
    Ok(paths::resolve(path, 0, paths::Kind::Program)?.real)
}
