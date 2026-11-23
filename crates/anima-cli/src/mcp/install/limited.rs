//! A program run with no shell, no input and no error output, for a limited time: an agent's own
//! program (MCP-035) or a program of the system that lists the members of a group.

use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What a program that ended wrote, and whether it ended without failure.
pub(super) struct Output {
    pub success: bool,
    /// At most one byte more than the `most` of [`run`]: more means that it wrote more.
    pub stdout: Vec<u8>,
}

/// Runs `command` for at most `limit` and keeps at most `most` bytes of its output, and one more
/// to show that there was more. `None` when it could not start or ran past `limit`: it is then
/// stopped.
pub(super) fn run(mut command: Command, limit: Duration, most: u64) -> Option<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(stdout) = stdout.as_mut() {
            let _ = stdout.take(most.saturating_add(1)).read_to_end(&mut out);
        }
        out
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < limit => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let stdout = reader.join().unwrap_or_default();
    status.map(|status| Output {
        success: status.success(),
        stdout,
    })
}
