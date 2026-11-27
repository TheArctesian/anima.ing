//! Project checkpoint traffic keeps its own slot and uses the root owner's stdio protocol.
use super::{Error, Result};
use agent_threads::managed::workspace::{
    Capacity,
    handoff::socket::{self, Command, Request},
};
use chrono::{TimeDelta, Utc};
use serde_json::{Value, json};
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
    process::{Child, ChildStdin, ChildStdout},
    sync::Semaphore,
};

const FRAME: u64 = 8192;
const REFUSED: Error = Error::Refused("The workspace checkpoint did not complete.");

pub(super) async fn run(uid: u32) -> Result<()> {
    let listener = super::listen(socket::PATH)?;
    let slot = Arc::new(Semaphore::new(1));
    loop {
        let (stream, _) = listener.accept().await?;
        if !super::authenticated(&stream, uid) {
            continue;
        }
        let Ok(held) = slot.clone().try_acquire_owned() else {
            continue;
        };
        tokio::spawn(async move {
            let _held = held;
            let _ = serve(stream).await;
        });
    }
}

async fn serve(stream: UnixStream) -> Result<()> {
    let (reading, mut writing) = stream.into_split();
    let mut reading = BufReader::new(reading);
    let frame = tokio::time::timeout(Duration::from_secs(5), read(&mut reading))
        .await
        .map_err(|_| REFUSED)??;
    let request: Request = serde_json::from_value(frame)?;
    let remaining = request.valid_until.signed_duration_since(Utc::now());
    if remaining > TimeDelta::hours(1) || remaining <= TimeDelta::zero() {
        return Err(REFUSED);
    }
    let limit = remaining
        .to_std()
        .map_err(|_| REFUSED)?
        .min(Duration::from_mins(21));
    tokio::time::timeout(limit, execute(request.command, &mut reading, &mut writing))
        .await
        .map_err(|_| REFUSED)?
}

async fn execute(
    command: Command,
    reading: &mut BufReader<OwnedReadHalf>,
    writing: &mut OwnedWriteHalf,
) -> Result<()> {
    let worker = Worker::open(&command).await?;
    match command {
        Command::Export { receipt } if receipt.fits(Capacity::MAXIMUM.storage_bytes) => {
            worker.export(writing, receipt.bytes).await
        }
        Command::Restore { import } if import.fits() => {
            worker.restore(reading, writing, import.receipt.bytes).await
        }
        Command::Export { .. } | Command::Restore { .. } => Err(REFUSED),
        _ => worker.control(writing).await,
    }
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Worker {
    async fn open(command: &Command) -> Result<Self> {
        let mut child = tokio::process::Command::new(std::env::current_exe()?)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut input = child.stdin.take().ok_or(REFUSED)?;
        let output = BufReader::new(child.stdout.take().ok_or(REFUSED)?);
        let mut bytes = serde_json::to_vec(command)?;
        bytes.push(b'\n');
        input.write_all(&bytes).await?;
        Ok(Self {
            child,
            input,
            output,
        })
    }

    async fn control(self, writing: &mut OwnedWriteHalf) -> Result<()> {
        let Self {
            mut child,
            input,
            mut output,
        } = self;
        drop(input);
        let frame = read(&mut output).await?;
        finished(&mut child).await?;
        send(writing, &frame).await
    }

    async fn export(self, writing: &mut OwnedWriteHalf, bytes: u64) -> Result<()> {
        let Self {
            mut child,
            input,
            mut output,
        } = self;
        drop(input);
        send(
            writing,
            &json!({"ok":true,"value":{"stream":socket::EXPORT}}),
        )
        .await?;
        let copied = io::copy(&mut (&mut output).take(bytes), writing).await?;
        if copied != bytes || output.read(&mut [0_u8; 1]).await? != 0 {
            return Err(REFUSED);
        }
        finished(&mut child).await?;
        send(writing, &json!({"ok":true,"value":{"exported":true}})).await
    }

    async fn restore(
        self,
        reading: &mut BufReader<OwnedReadHalf>,
        writing: &mut OwnedWriteHalf,
        bytes: u64,
    ) -> Result<()> {
        let Self {
            mut child,
            mut input,
            mut output,
        } = self;
        let first = read(&mut output).await?;
        if first["ok"] != true || first["value"]["stream"] != socket::RESTORE {
            drop(input);
            finished(&mut child).await?;
            return send(writing, &first).await;
        }
        send(writing, &first).await?;
        let copied = io::copy(&mut reading.take(bytes + 1), &mut input).await?;
        if copied != bytes {
            return Err(REFUSED);
        }
        drop(input);
        let terminal = read(&mut output).await?;
        finished(&mut child).await?;
        send(writing, &terminal).await
    }
}

async fn finished(child: &mut Child) -> Result<()> {
    if child.wait().await?.success() {
        Ok(())
    } else {
        Err(REFUSED)
    }
}

async fn read(input: &mut (impl AsyncBufReadExt + Unpin)) -> Result<Value> {
    let mut frame = Vec::new();
    input.take(FRAME + 1).read_until(b'\n', &mut frame).await?;
    if frame.len() as u64 > FRAME || frame.last() != Some(&b'\n') {
        return Err(REFUSED);
    }
    Ok(serde_json::from_slice(&frame)?)
}

async fn send(output: &mut OwnedWriteHalf, reply: &Value) -> Result<()> {
    let mut frame = serde_json::to_vec(reply)?;
    frame.push(b'\n');
    if frame.len() as u64 > FRAME {
        return Err(REFUSED);
    }
    output.write_all(&frame).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
