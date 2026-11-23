//! Whether a client of the runner is a process of an agent: a process below the holder, a
//! process that carries the mark of a thread, or a process that came from the runner and left
//! its tree (THR-049). Such a client may read and write to a thread, but it may not answer a
//! request, raise the limit or start a thread: an agent with a shell must not approve itself
//! (Medium 3 of the checklist read).
//!
//! A process can leave the tree (a new session and a second start make the first process of
//! the system its parent) and its mark (a new environment). Its group of origin goes with it:
//! the system gives it to each process that a process starts. On a Mac that is the resource
//! coalition that launchd gives to each job (the runner of the installed application, an
//! application such as the Terminal), and only launchd changes it. On Linux it is the control
//! group (the unit of the runner). So a client in the group of the runner is of an agent, unless
//! the runner is that client or a process above the runner in that group started it (in a test,
//! the test). A process that the runner started itself (the ask for a version, the login shell)
//! is not the person: its program is a file that an agent can change.
//!
//! On Linux a systemd user unit with a mount option (`ProtectHome=`, `PrivateTmp=`) puts the
//! runner in a user namespace of its own, and from there the environment of each process outside
//! it cannot be read. Each process below the holder is in that namespace or in one that it made:
//! a process cannot enter a namespace above its own. So a client in a namespace above the runner's
//! is not below the holder and did not take the mark from it; it goes on to the check of its
//! group. A client that the runner cannot read for any other reason is still of an agent.
//!
//! Known limit: this makes a call of an agent harder, not impossible. A process of an agent with
//! full access can ask any program of the person outside the group to make the call (launchd,
//! systemd, cron, a terminal by its script interface, a `tmux` server, `ssh` to this computer).
//! On Linux it can also move itself to a different control group of the user, as systemd gives
//! the user the groups below `user@.service`.

// The feature `test-person` makes the process that started the runner the person, for the
// integration tests only: a release build must never have it.
#[cfg(all(feature = "test-person", not(debug_assertions)))]
compile_error!("the feature test-person is for the tests of the daemon, never for a release build");

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

/// The longest wait for the group of one process.
const ORIGIN_WAIT: Duration = Duration::from_secs(2);

/// Whether the process `pid` is of an agent of the holder whose folder is `holder_dir`.
/// When the answer cannot be known, the client is taken for an agent: a peer with no process
/// ID, a process that ended before it was read (a child that connects and exits), or one whose
/// environment or group cannot be read. Such a client can still read and write; only the
/// person's calls are refused (R-315).
pub async fn of_an_agent(pid: Option<i32>, holder_dir: &Path) -> bool {
    let Some(pid) = pid
        .and_then(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid > 1)
    else {
        return true;
    };
    let mark = crate::threads::marker::mark_of(pid).await;
    #[cfg(all(feature = "test-person", unix))]
    if pid == std::os::unix::process::parent_id() {
        return started_the_runner(pid, mark, holder_dir).await;
    }
    if mark == Some(true) || (mark.is_none() && !above_the_runner(pid).await) {
        return true;
    }
    let Some(table) = commands().await else {
        return true;
    };
    if !table.contains_key(&pid) || below_the_holder(&table, pid, &holder_mark(holder_dir)) {
        return true;
    }
    if !GROUPS_KNOWN {
        return false;
    }
    let me = std::process::id();
    let Ok(mine) = RUNNER
        .get_or_try_init(|| async { origin(me).await.ok_or(()) })
        .await
    else {
        return true;
    };
    match origin(pid).await {
        Some(theirs) if theirs != *mine => false,
        // The line is read from the table of this call: a process that took the ID of a
        // parent that ended is not above the runner now.
        Some(_) => !comes_from(&table, pid, me, &line_of(&table, me, mine).await),
        None => true,
    }
}

/// The process that started the runner, with the feature `test-person` (the integration
/// tests): it is the person, though a read of it fails, unless it is of an agent by what can be
/// read (the mark of a thread, a place below the holder).
#[cfg(all(feature = "test-person", unix))]
async fn started_the_runner(pid: u32, mark: Option<bool>, holder_dir: &Path) -> bool {
    let below = commands()
        .await
        .is_some_and(|table| below_the_holder(&table, pid, &holder_mark(holder_dir)));
    mark == Some(true) || below
}

/// Whether the user namespace of the process `pid` holds a user ID that the namespace of the
/// runner does not have: the system writes such an ID in `uid_map` as 4294967295. A namespace of
/// the runner, or one below it, holds only IDs of the runner's, so such a process is not of the
/// holder. False when the map cannot be read, and where the system has no user namespaces.
#[cfg_attr(
    not(target_os = "linux"),
    expect(
        clippy::unused_async,
        reason = "only Linux reads the map; the callers await it on every system"
    )
)]
async fn above_the_runner(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let read = tokio::fs::read_to_string(format!("/proc/{pid}/uid_map"));
        tokio::time::timeout(ORIGIN_WAIT, read)
            .await
            .ok()
            .and_then(Result::ok)
            .is_some_and(|map| {
                map.lines()
                    .any(|line| line.split_whitespace().nth(1) == Some("4294967295"))
            })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

/// The group of origin of the runner, which does not change while it runs; a read that failed
/// is tried again at the next client.
static RUNNER: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

/// Whether this system gives each process a group of origin that [`origin`] can read.
const GROUPS_KNOWN: bool = cfg!(any(target_os = "macos", target_os = "linux"));

/// The runner and each process above it that has the same group: the processes that may start a
/// client of the person in that group (the runner itself, and in a test the test).
async fn line_of(table: &Commands, me: u32, mine: &str) -> BTreeSet<u32> {
    let mut line = BTreeSet::from([me]);
    let mut at = me;
    for _ in 0..64 {
        let Some((parent, _)) = table.get(&at) else {
            break;
        };
        if *parent <= 1 || origin(*parent).await.as_deref() != Some(mine) {
            break;
        }
        line.insert(*parent);
        at = *parent;
    }
    line
}

/// Whether `pid` is the runner `me`, or a process of `line` above the runner started it. A
/// process that the runner itself started does not come from the line.
#[must_use]
pub fn comes_from(table: &Commands, pid: u32, me: u32, line: &BTreeSet<u32>) -> bool {
    if pid == me {
        return true;
    }
    let mut at = pid;
    for _ in 0..64 {
        if at == me {
            return false;
        }
        if line.contains(&at) {
            return true;
        }
        match table.get(&at) {
            Some((parent, _)) if *parent > 1 => at = *parent,
            _ => return false,
        }
    }
    false
}

/// The group of origin of the process `pid`, as the system keeps it; `None` when it cannot be
/// read.
async fn origin(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let run = tokio::process::Command::new("/bin/launchctl")
            .args(["print", &format!("pid/{pid}")])
            .env_clear()
            .env("LC_ALL", "C")
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(ORIGIN_WAIT, run).await.ok()?.ok()?;
        coalition(&String::from_utf8_lossy(&output.stdout))
    }
    #[cfg(target_os = "linux")]
    {
        let read = tokio::fs::read_to_string(format!("/proc/{pid}/cgroup"));
        let text = tokio::time::timeout(ORIGIN_WAIT, read).await.ok()?.ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (pid, ORIGIN_WAIT);
        None
    }
}

/// The ID of the resource coalition in the text of `launchctl print pid/<pid>`.
#[must_use]
pub fn coalition(text: &str) -> Option<String> {
    let mut lines = text
        .lines()
        .skip_while(|line| line.trim() != "resource coalition = {");
    lines.next()?;
    lines
        .take_while(|line| line.trim() != "}")
        .find_map(|line| line.trim().strip_prefix("ID = "))
        .map(str::to_owned)
}

/// The end of the command of the holder of `holder_dir`, as `ps` shows it.
fn holder_mark(holder_dir: &Path) -> String {
    format!(" holder {}", holder_dir.display())
}

/// Each process with its parent and its command.
type Commands = BTreeMap<u32, (u32, String)>;

async fn commands() -> Option<Commands> {
    let output = crate::threads::tree::ps_output(&["-axww", "-o", "pid=,ppid=,command="]).await?;
    Some(table(&String::from_utf8_lossy(&output)))
}

/// The rows of `ps -axww -o pid=,ppid=,command=`; a line that does not read is left out.
#[must_use]
pub fn table(text: &str) -> Commands {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, rest) = line.split_once(char::is_whitespace)?;
            let rest = rest.trim_start();
            let (parent, command) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Some((
                pid.parse().ok()?,
                (parent.parse().ok()?, command.trim().to_owned()),
            ))
        })
        .collect()
}

/// Whether an ancestor of `pid` is the holder: its command ends with `holder_mark`.
#[must_use]
pub fn below_the_holder(table: &Commands, pid: u32, holder_mark: &str) -> bool {
    let mut at = pid;
    for _ in 0..64 {
        let Some((parent, _)) = table.get(&at) else {
            return false;
        };
        if *parent <= 1 {
            return false;
        }
        if table
            .get(parent)
            .is_some_and(|(_, command)| command.ends_with(holder_mark))
        {
            return true;
        }
        at = *parent;
    }
    false
}

#[cfg(test)]
mod tests;
