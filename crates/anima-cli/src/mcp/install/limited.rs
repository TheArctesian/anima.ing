//! A program run with no shell, no input and no error output, for a limited time: an agent's own
//! program (MCP-035) or a program of the system that lists the members of a group.

use std::io::Read as _;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the output of a program that ended may take to close, at the least: a program it
/// started can keep it open.
const CLOSE: Duration = Duration::from_secs(1);

/// What a program that ended wrote, and whether it ended without failure.
pub(super) struct Output {
    pub success: bool,
    /// At most one byte more than the `most` of [`run`]: more means that it wrote more.
    pub stdout: Vec<u8>,
}

/// Runs `command` for at most `limit` and keeps at most `most` bytes of its output, and one more
/// to show that there was more. `None` when it could not start, or it or a program that it started
/// ran past `limit`: they are then stopped.
pub(super) fn run(mut command: Command, limit: Duration, most: u64) -> Option<Output> {
    // Its own process group, so that each program it starts can be stopped with it.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(stdout) = stdout.as_mut() {
            let _ = stdout.take(most.saturating_add(1)).read_to_end(&mut out);
        }
        let _ = sender.send(out);
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < limit => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                stop(&mut child);
                return None;
            }
        }
    };
    let Ok(stdout) = receiver.recv_timeout(limit.saturating_sub(start.elapsed()).max(CLOSE)) else {
        stop(&mut child);
        return None;
    };
    Some(Output {
        success: status.success(),
        stdout,
    })
}

/// Stops `child` and each program in its process group.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(group) = i32::try_from(child.id()) {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(group),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Command;
    use std::time::{Duration, Instant};

    fn shell(line: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", line]);
        command
    }

    #[test]
    fn mcp_035_a_program_that_leaves_a_program_holding_its_output_is_stopped_at_the_limit() {
        let start = Instant::now();
        let left = super::run(
            shell("/bin/sleep 60 & echo started"),
            Duration::from_secs(2),
            100,
        );
        assert!(left.is_none());
        assert!(start.elapsed() < Duration::from_secs(30));
        // A program that ends and closes its output is read whole.
        let output =
            super::run(shell("echo done"), Duration::from_secs(30), 100).expect("it ended");
        assert!(output.success && output.stdout == b"done\n");
    }
}
