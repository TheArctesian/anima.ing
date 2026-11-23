//! A program run with no shell, no input and no error output, for a limited time: an agent's own
//! program (MCP-035) or a program of the system that lists the members of a group.

use std::io::Read as _;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long the output of a program that ended may take to close, at the least: a program that it
/// started can keep it open. Such a program is not stopped, and the output is not waited for.
const CLOSE: Duration = Duration::from_secs(1);

/// What a program that ended wrote, and whether it ended without failure.
pub(super) struct Output {
    pub success: bool,
    /// At most one byte more than the `most` of [`run`]: more means that it wrote more.
    pub stdout: Vec<u8>,
}

/// Runs `command` for at most `limit` and keeps at most `most` bytes of its output, and one more
/// to show that there was more. `None` when it could not start, ran past `limit` (it is then
/// stopped), or left its output open past `limit`.
pub(super) fn run(mut command: Command, limit: Duration, most: u64) -> Option<Output> {
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
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let Ok(stdout) = receiver.recv_timeout(limit.saturating_sub(start.elapsed()).max(CLOSE)) else {
        return None;
    };
    Some(Output {
        success: status.success(),
        stdout,
    })
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
    fn mcp_035_a_program_that_leaves_a_program_holding_its_output_is_not_waited_for_past_the_limit()
    {
        let start = Instant::now();
        let left = super::run(
            shell("/bin/sleep 20 & echo started"),
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
