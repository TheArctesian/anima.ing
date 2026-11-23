//! One cgroup v2 for each server (Linux, R-331), below the cgroup of the daemon, which its unit
//! must delegate. The server joins it through the launcher: the daemon program in the mode
//! `__model-launch`, or the root-owned model launcher where `AppArmor` restricts user namespaces
//! (crate `model-launcher`; L1-6c). No shell runs.

use std::io;
use std::path::{Path, PathBuf};

use daemon_core::model_work::cause::Containment;

use super::Identity;

pub const LAUNCH_MODE: &str = "__model-launch";
pub const PREFIX: &str = "anima-model-";
/// A placeholder value of the contract (Section 9), so a fork bomb stops before a stop loses to
/// it on a kernel with no `cgroup.kill`.
pub const PIDS_MAX: u32 = 512;

/// Field 22 of `/proc/<pid>/stat`, in clock ticks.
#[must_use]
pub fn start_of(pid: i32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(')')?.1;
    after.split_whitespace().nth(19).map(str::to_owned)
}

#[must_use]
pub fn own_cgroup() -> Option<PathBuf> {
    let text = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let relative = text.lines().find_map(|line| line.strip_prefix("0::"))?;
    let base = Path::new("/sys/fs/cgroup");
    base.join("cgroup.controllers")
        .exists()
        .then(|| base.join(relative.trim_start_matches('/')))
}

/// Each start has its own name, so a cgroup of an earlier start that could not be removed never
/// blocks a new start. With `nsdelegate` the limits are not writable from inside the runtime.
///
/// The unit of the daemon must have `Delegate=yes` (DEF-013): under the systemd user
/// manager each unit cgroup belongs to the user, so a cgroup that can be made proves nothing.
///
/// # Errors
/// `no_nsdelegate` when the cgroup v2 mount lacks `nsdelegate`; `no_delegation` otherwise.
pub fn make(name: &str) -> Result<PathBuf, Containment> {
    let mountinfo =
        std::fs::read_to_string("/proc/self/mountinfo").map_err(|_| Containment::NoDelegation)?;
    if !super::mounts::cgroup_has_nsdelegate(&mountinfo) {
        return Err(Containment::NoNsdelegate);
    }
    if !super::delegation::own_cgroup_delegated() {
        return Err(Containment::NoDelegation);
    }
    let parent = own_cgroup().ok_or(Containment::NoDelegation)?;
    let start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = parent.join(format!("{PREFIX}{name}-{start}"));
    std::fs::create_dir(&path).map_err(|_| Containment::NoDelegation)?;
    let limits = std::fs::write(path.join("cgroup.max.descendants"), "0").and_then(|()| {
        let pids = path.join("pids.max");
        if pids.exists() {
            std::fs::write(&pids, PIDS_MAX.to_string())
        } else {
            Ok(())
        }
    });
    if limits.is_err() {
        let _ = std::fs::remove_dir(&path);
        return Err(Containment::NoDelegation);
    }
    Ok(path)
}

#[must_use]
pub fn is_server_cgroup(cgroup: &Path) -> bool {
    let named = cgroup
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PREFIX));
    named && own_cgroup().is_some_and(|own| cgroup.parent() == Some(own.as_path()))
}

/// # Errors
/// An I/O error when a `cgroup.procs` or a folder does not read; the caller then fails closed.
pub fn members(cgroup: &Path) -> io::Result<Vec<Identity>> {
    let mut found = Vec::new();
    for folder in subtree(cgroup)? {
        let text = std::fs::read_to_string(folder.join("cgroup.procs"))?;
        found.extend(
            text.lines()
                .filter_map(|line| line.trim().parse().ok())
                .filter_map(super::identity),
        );
    }
    Ok(found)
}

/// Parents before children; no link is followed.
fn subtree(cgroup: &Path) -> io::Result<Vec<PathBuf>> {
    let mut folders = vec![cgroup.to_path_buf()];
    let mut next = 0;
    while let Some(folder) = folders.get(next).cloned() {
        next += 1;
        for entry in std::fs::read_dir(&folder)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                folders.push(entry.path());
            }
        }
    }
    Ok(folders)
}

/// Waits up to 1 second, so it runs on a blocking thread. A cgroup that does not empty is left;
/// the next start uses a new name.
pub fn end_cgroup(cgroup: &Path) {
    let _ = std::fs::write(cgroup.join("cgroup.kill"), "1");
    for member in members(cgroup).unwrap_or_default() {
        let _gone_or_sent = super::signal(&member, rustix::process::Signal::KILL);
    }
    for _ in 0..50 {
        // Children first: a cgroup that holds a cgroup cannot be removed.
        for folder in subtree(cgroup).unwrap_or_default().iter().rev() {
            let _ = std::fs::remove_dir(folder);
        }
        if !cgroup.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    tracing::warn!(cgroup = %cgroup.display(), "a cgroup of a model server did not empty; left");
}

pub fn sweep_cgroups() {
    let Some(parent) = own_cgroup() else { return };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with(PREFIX) {
            end_cgroup(&entry.path());
        }
    }
}

/// Becomes the launcher when the first argument is [`LAUNCH_MODE`]; otherwise returns. Called
/// before any thread starts, because a new namespace needs one thread.
pub fn dispatch_if_launch() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some(LAUNCH_MODE) {
        launch(&arguments[1..]);
    }
}

/// `__model-launch <cgroup.procs> <daemon pid> <failure file> <program> <arguments…>`, the steps
/// of [`model_launcher::launch::run`] with no signed list.
///
/// The parent-death signal fires when the thread that started the launcher ends, not the daemon
/// process, so the daemon starts servers from a worker of the async runtime, not `spawn_blocking`.
pub fn launch(arguments: &[String]) -> ! {
    let Some(order) = model_launcher::launch::Order::parse(arguments) else {
        std::process::exit(model_launcher::code::Step::Exec.code());
    };
    model_launcher::launch::run(&order, None)
}
