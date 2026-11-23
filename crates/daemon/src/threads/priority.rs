//! The programs that the person waits for run at normal priority (R-403). The
//! daemon is a launchd agent of background priority, and each program it starts inherits that:
//! on a busy Mac, `codex app-server` took up to 79 s for its first answer (measured
//! 2026-11-22, `side-session-notes/agent-priority-measured.md`). The runner lifts the holder,
//! which starts the agent programs, and its own short questions to a program, out of the
//! background state, with `/usr/sbin/taskpolicy -B -p <pid>`: the daemon's own work stays in
//! the background. The pid is always one of a child the runner started and holds, never one
//! from a file or a message. A lift that fails changes nothing else: the program runs at
//! background priority, and the runner log says so once.
//!
//! Linux: the systemd units set no priority, so nothing is lifted.

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::time::Duration;

/// The program that lifts, by its absolute path, with fixed arguments around the pid.
#[cfg(target_os = "macos")]
const TASKPOLICY: &str = "/usr/sbin/taskpolicy";

/// The longest wait for a lift. `taskpolicy` itself starts at background priority, so on a
/// busy Mac it can be slow to start.
#[cfg(target_os = "macos")]
const LIFT_WAIT: Duration = Duration::from_secs(15);

/// A lift failed since the daemon started.
static FAILED: AtomicBool = AtomicBool::new(false);
/// The runner log said so.
static SAID: AtomicBool = AtomicBool::new(false);

/// Lifts `child`, which the caller started and holds and has not waited for, so that its pid
/// cannot be one of a different process. Blocks for at most [`LIFT_WAIT`]: call it from a
/// thread that may block. A lift that fails while the child still runs is kept for
/// [`failure_to_say`]; a child that ended before the lift needs none.
pub fn lift(child: &mut std::process::Child) {
    if !run(child.id()) && matches!(child.try_wait(), Ok(None)) {
        FAILED.store(true, Ordering::Relaxed);
    }
}

/// [`lift`] for a child of tokio. The child is held, not waited for, while the lift runs.
pub async fn lift_async(child: &mut tokio::process::Child) {
    let Some(pid) = child.id() else {
        return;
    };
    let done = tokio::task::spawn_blocking(move || run(pid))
        .await
        .unwrap_or(false);
    if !done && matches!(child.try_wait(), Ok(None)) {
        FAILED.store(true, Ordering::Relaxed);
    }
}

/// Whether the runner log must say now that a lift failed: true one time for each start of
/// the daemon.
pub fn failure_to_say() -> bool {
    FAILED.load(Ordering::Relaxed) && !SAID.swap(true, Ordering::Relaxed)
}

#[cfg(target_os = "macos")]
fn run(pid: u32) -> bool {
    use std::process::{Command, Stdio};
    use std::time::Instant;
    let Ok(mut child) = Command::new(TASKPOLICY)
        .args(["-B", "-p"])
        .arg(pid.to_string())
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let end = Instant::now() + LIFT_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
            // Past the deadline, or the wait failed: no lift, and a failed kill of a program
            // that is ending adds nothing to it.
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
const fn run(_pid: u32) -> bool {
    true
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn r_new_agentpriority_a_held_child_is_lifted_and_a_pid_of_no_process_is_not() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .expect("a child");
        assert!(run(child.id()), "a child of this user is lifted");
        lift(&mut child);
        let _ = child.kill();
        let _ = child.wait();
        // Above the highest pid of macOS (99 999), so no process can have it.
        assert!(!run(4_000_000), "a pid of no process is not");
    }
}
