//! The daemon side of the fetch process: a stall or a fault of the child ends the child, never
//! the daemon, and reads as `fetch_failed` (R-406).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::{Ending, FetchOrder, MAX_REPLY_BYTES, verdict};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::process::{Child, Command};

use super::child::FETCH_MODE;

/// # Errors
/// The cause that the child gave back, or `fetch_failed` for each other end.
pub async fn fetch_in_child(
    program: &Path,
    order: &FetchOrder,
    deadline: Duration,
) -> Result<(), Cause> {
    let Some(mut child) = start(program) else {
        return verdict(Ending::Fault);
    };
    let ending = tokio::time::timeout(deadline, reply_of(&mut child, order)).await;
    if !matches!(ending, Ok(Some(_))) {
        // `kill` also waits, so no process of the child is left behind.
        let _ = child.kill().await;
    }
    match ending {
        Err(_) => verdict(Ending::Stalled),
        Ok(None) => verdict(Ending::Fault),
        Ok(Some(text)) => verdict(Ending::Exited(&text)),
    }
}

/// Called on a worker of the async runtime, not in `spawn_blocking`: on Linux the parent-death
/// signal of the child fires when the spawning thread exits.
fn start(program: &Path) -> Option<Child> {
    Command::new(program)
        .arg(FETCH_MODE)
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()
}

async fn reply_of(child: &mut Child, order: &FetchOrder) -> Option<String> {
    let mut text = serde_json::to_vec(order).ok()?;
    text.push(b'\n');
    // The input stays open while the child runs, as the child ends when it closes; a failed
    // write shows in the child's exit.
    let mut input = child.stdin.take()?;
    let _ = input.write_all(&text).await;
    let mut reply = String::new();
    child
        .stdout
        .take()?
        .take(MAX_REPLY_BYTES)
        .read_to_string(&mut reply)
        .await
        .ok()?;
    let status = child.wait().await.ok()?;
    drop(input);
    status.success().then_some(reply)
}
