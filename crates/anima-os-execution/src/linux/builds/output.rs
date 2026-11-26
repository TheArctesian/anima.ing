//! Explicit-owner diagnostics, isolated from receipts and routine inventory.
pub(super) mod progress;
use self::progress::Stage;
use super::{
    super::{Error, Result},
    store,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs, io::Write, os::unix::fs::OpenOptionsExt, path::Path, process::Stdio, time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
use uuid::Uuid;

const LIMIT: usize = 65_536;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    schema: u32,
    program: String,
    stage: Stage,
    finished: bool,
    stdout: String,
    stderr: String,
}

pub async fn run(program: &str, arguments: &[&str], seconds: u64, stage: Stage) -> Result<()> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("Build output is unavailable."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(Error::Refused("Build output is unavailable."))?;
    let result = tokio::time::timeout(Duration::from_secs(seconds), async {
        tokio::try_join!(child.wait(), capture(stdout), capture(stderr))
    })
    .await;
    let (status, stdout, stderr) = if let Ok(result) = result {
        result?
    } else {
        save(
            Path::new("/work"),
            &Output {
                schema: 1,
                program: program.into(),
                stage,
                finished: false,
                stdout: String::new(),
                stderr:
                    "The build stage exceeded its deadline; incomplete output was not retained."
                        .into(),
            },
        )?;
        return Err(Error::Refused("The build stage exceeded its deadline."));
    };
    save(
        Path::new("/work"),
        &Output {
            schema: 1,
            program: program.into(),
            stage,
            finished: true,
            stdout,
            stderr,
        },
    )?;
    if !status.success() {
        return Err(Error::Refused("The isolated build stage failed."));
    }
    Ok(())
}

async fn capture(mut input: impl AsyncRead + Unpin) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut block = [0; 4096];
    let mut oversized = false;
    loop {
        let count = input.read(&mut block).await?;
        if count == 0 {
            break;
        }
        if !oversized && bytes.len() + count <= LIMIT {
            bytes.extend_from_slice(&block[..count]);
        } else {
            oversized = true;
            bytes.clear();
        }
    }
    if oversized {
        return Ok("Output omitted: stream exceeded the diagnostic limit.".into());
    }
    Ok(sanitize(&String::from_utf8_lossy(&bytes)))
}

fn sanitize(stream: &str) -> String {
    // Redact the complete bounded stream, including multiline keys, before clipping.
    let text = redaction::redact_text(stream);
    if text.len() > LIMIT {
        return "Output omitted: redacted stream exceeded the diagnostic limit.".into();
    }
    text.chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect()
}

fn save(work: &Path, output: &Output) -> Result<()> {
    let pending = work.join("output.pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(output)?)?;
    file.sync_all()?;
    fs::rename(pending, work.join("output.json"))?;
    fs::File::open(work)?.sync_all()?;
    Ok(())
}

pub fn read(id: Uuid) -> Result<Value> {
    let _attempt = store::lock(id)?;
    let attempt = store::required(id)?;
    if attempt.phase == "deleted" {
        return Err(Error::Refused(
            "This build's diagnostics have been released.",
        ));
    }
    let output: Output = store::read(&store::base(id).join("work/output.json"))?;
    if output.schema != 1
        || !matches!(output.program.as_str(), "buildctl" | "railpack")
        || output.stdout.len() > LIMIT
        || output.stderr.len() > LIMIT
    {
        return Err(Error::Refused("The build diagnostics are invalid."));
    }
    // The worker owns this file: redact again at the root boundary before delivery.
    Ok(
        serde_json::json!({"attempt":id,"program":output.program,"stage":output.stage,"finished":output.finished,
        "stdout":sanitize(&output.stdout),"stderr":sanitize(&output.stderr)}),
    )
}

#[cfg(test)]
mod tests;
