use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use rustix::process::{Signal, getuid};
use uuid::Uuid;

use crate::threads::{marker::MARK, tree::identity::Process};

pub(in super::super) async fn sweep(runner: Uuid) -> bool {
    tokio::time::timeout(Duration::from_secs(2), end_marked(runner))
        .await
        .unwrap_or(false)
}

async fn end_marked(runner: Uuid) -> bool {
    let wanted = format!("{MARK}={runner}");
    loop {
        match empty_after_sweep(&wanted).await {
            Some(true) => return true,
            Some(false) => {}
            None => return false,
        }
    }
}

async fn empty_after_sweep(wanted: &str) -> Option<bool> {
    let mut folders = tokio::fs::read_dir("/proc").await.ok()?;
    let mut complete = true;
    let mut signalled = Vec::new();
    loop {
        let folder = match folders.next_entry().await {
            Ok(Some(folder)) => folder,
            Ok(None) => break,
            Err(_) => return None,
        };
        let Some(pid) = folder
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        match marked(&folder.path(), wanted).await {
            Some(true) => {
                if let Some(process) = Process::open(pid)
                    && marked(&folder.path(), wanted).await == Some(true)
                    && process.live()
                    && process.signal(Signal::KILL).is_ok()
                {
                    signalled.push(process);
                } else {
                    complete = false;
                }
            }
            None => complete = false,
            Some(false) => {}
        }
    }
    while !signalled.iter().all(Process::terminated) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    complete.then_some(signalled.is_empty())
}

async fn marked(folder: &Path, wanted: &str) -> Option<bool> {
    match tokio::fs::read(folder.join("environ")).await {
        Ok(bytes) => Some(
            bytes
                .split(|byte| *byte == 0)
                .any(|entry| entry == wanted.as_bytes()),
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => Some(false),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            let status = tokio::fs::read_to_string(folder.join("status"))
                .await
                .ok()?;
            let uid = status
                .lines()
                .find_map(|line| line.strip_prefix("Uid:"))?
                .split_whitespace()
                .next()?
                .parse::<u32>()
                .ok()?;
            (uid != getuid().as_raw()).then_some(false)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests;
