use agent_threads::managed::workspace::handoff::{
    Receipt,
    socket::{self, Command, Request},
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use serde_json::Value;
use std::{io, time::Duration};
use tokio::{
    io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader},
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::oneshot,
};

const FRAME: u64 = 8192;
const CHUNK: usize = 64 * 1024;

pub(super) async fn open(
    command: Command,
    valid_until: DateTime<Utc>,
) -> Option<(BufReader<OwnedReadHalf>, OwnedWriteHalf)> {
    let stream = tokio::time::timeout(Duration::from_secs(5), UnixStream::connect(socket::PATH))
        .await
        .ok()?
        .ok()?;
    if stream.peer_cred().ok()?.uid() != 0 {
        return None;
    }
    let (input, mut output) = stream.into_split();
    let mut frame = serde_json::to_vec(&Request {
        valid_until,
        command,
    })
    .ok()?;
    frame.push(b'\n');
    if frame.len() as u64 > FRAME {
        return None;
    }
    output.write_all(&frame).await.ok()?;
    Some((BufReader::new(input), output))
}

pub(super) async fn control(command: Command, valid_until: DateTime<Utc>) -> Option<Value> {
    let (mut input, mut output) = open(command, valid_until).await?;
    output.shutdown().await.ok()?;
    reply(&mut input).await
}

pub(super) async fn reply(input: &mut BufReader<OwnedReadHalf>) -> Option<Value> {
    let mut frame = Vec::new();
    input
        .take(FRAME + 1)
        .read_until(b'\n', &mut frame)
        .await
        .ok()?;
    if frame.len() as u64 > FRAME || frame.last() != Some(&b'\n') {
        return None;
    }
    let mut value: Value = serde_json::from_slice(&frame).ok()?;
    (value["ok"] == true).then(|| value["value"].take())
}

pub(super) async fn export(
    receipt: &Receipt,
    valid_until: DateTime<Utc>,
) -> Option<(crate::hub::handoffs::Stream, oneshot::Receiver<()>)> {
    let (mut input, mut output) = open(
        Command::Export {
            receipt: receipt.clone(),
        },
        valid_until,
    )
    .await?;
    output.shutdown().await.ok()?;
    if reply(&mut input).await?["stream"] != socket::EXPORT {
        return None;
    }
    let (complete, confirmed) = oneshot::channel();
    let export = Export {
        input,
        remaining: receipt.bytes,
        complete,
    };
    Some((
        futures_util::stream::try_unfold(export, Export::chunk).boxed(),
        confirmed,
    ))
}

struct Export {
    input: BufReader<OwnedReadHalf>,
    remaining: u64,
    complete: oneshot::Sender<()>,
}

impl Export {
    async fn chunk(mut self) -> io::Result<Option<(Bytes, Self)>> {
        if self.remaining == 0 {
            let result = reply(&mut self.input).await;
            if result.is_none_or(|value| value["exported"] != true) {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let _ = self.complete.send(());
            return Ok(None);
        }
        let limit = usize::try_from(self.remaining).unwrap_or(CHUNK).min(CHUNK);
        let mut bytes = vec![0_u8; limit];
        let count = self.input.read(&mut bytes).await?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.remaining -= count as u64;
        bytes.truncate(count);
        Ok(Some((bytes.into(), self)))
    }
}
