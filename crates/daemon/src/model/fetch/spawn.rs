//! The daemon side of the fetch process: it starts the child with an empty environment, writes
//! its order, reads its one word and ends it at the deadline. A stall or a fault of the child
//! ends the child, never the daemon (R-NEW-fetchfault).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::{Ending, FetchOrder, MAX_REPLY_BYTES, verdict};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::process::{Child, Command};

use super::child::FETCH_MODE;

/// Runs `order` in a child made from `program`, and waits at most `deadline`.
///
/// # Errors
/// The cause that the child gave back; `source_failed` for a stall, a fault or a reply that
/// does not read.
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

/// Starts `program` in the fetch mode with an empty environment.
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

/// The reply of a child that exited with code 0; `None` for each other end.
async fn reply_of(child: &mut Child, order: &FetchOrder) -> Option<String> {
    let text = serde_json::to_vec(order).ok()?;
    let mut input = child.stdin.take()?;
    // A child that ends before it reads its order is a fault; its exit status says so.
    let _ = input.write_all(&text).await;
    drop(input);
    let mut reply = String::new();
    child
        .stdout
        .take()?
        .take(MAX_REPLY_BYTES)
        .read_to_string(&mut reply)
        .await
        .ok()?;
    let status = child.wait().await.ok()?;
    status.success().then_some(reply)
}
