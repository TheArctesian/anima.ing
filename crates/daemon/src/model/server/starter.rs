//! What starts the runtime program of a server: the program itself (macOS), or on Linux a
//! launcher that first puts it in the cgroup of the server (R-331): the daemon program in its
//! launch mode, or the root-owned model launcher where `AppArmor` restricts user namespaces
//! (contract K11 Section 9.1; L1-6c).

use std::path::PathBuf;
use std::process::Stdio;

use daemon_core::model_work::cause::Cause;
use tokio::process::Command;

use super::Launch;
#[cfg(target_os = "linux")]
use crate::model::tree::launcher::Way;

/// The start of one server.
pub enum Starter {
    /// The runtime program itself, with no launcher.
    #[cfg_attr(target_os = "linux", expect(dead_code, reason = "macOS only"))]
    Program,
    /// A launcher that joins `cgroup` and then becomes the runtime.
    #[cfg(target_os = "linux")]
    Launcher { cgroup: PathBuf, way: Way },
}

impl Starter {
    /// The cgroup of the server, on Linux.
    pub fn cgroup(&self) -> Option<PathBuf> {
        match self {
            Self::Program => None,
            #[cfg(target_os = "linux")]
            Self::Launcher { cgroup, .. } => Some(cgroup.clone()),
        }
    }
}

/// The cgroup of the server and its launcher on Linux; the program itself on macOS.
///
/// # Errors
/// `containment_failed` with the step that failed; `runtime_not_signed` where the model
/// launcher is used and the program is not on the kept list.
#[cfg_attr(
    not(target_os = "linux"),
    expect(
        clippy::unused_async,
        reason = "only the checks of the Linux launcher await"
    )
)]
pub async fn contain(launch: &Launch) -> Result<Starter, Cause> {
    #[cfg(target_os = "linux")]
    {
        use crate::model::tree::{launcher, linux};
        let way = launcher::way(&launch.program, &launch.state).await?;
        let cgroup = linux::make(&launch.server.to_string()).map_err(Cause::ContainmentFailed)?;
        Ok(Starter::Launcher { cgroup, way })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = launch;
        Ok(Starter::Program)
    }
}

/// The command of `launch` through `starter`, with the empty environment and the fixed list.
pub fn command(launch: &Launch, starter: &Starter) -> Command {
    let mut command = match starter {
        Starter::Program => Command::new(&launch.program),
        #[cfg(target_os = "linux")]
        Starter::Launcher { cgroup, way } => {
            let mut command = match way {
                Way::Daemon => {
                    let mut command = Command::new(&launch.launcher);
                    command.arg(crate::model::tree::linux::LAUNCH_MODE);
                    command
                }
                Way::Root { list, signature } => {
                    let mut command = Command::new(model_launcher::PATH);
                    command.arg(list).arg(signature);
                    command
                }
            };
            command
                .arg(cgroup.join("cgroup.procs"))
                .arg(std::process::id().to_string())
                .arg(launch.place.join(crate::model::tree::LAUNCH_FAILURE))
                .arg(&launch.program);
            command
        }
    };
    command
        .args(&launch.arguments)
        .env_clear()
        .envs(launch.environment.iter().map(|(name, value)| (name, value)))
        .current_dir(&launch.temporary)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    command
}
