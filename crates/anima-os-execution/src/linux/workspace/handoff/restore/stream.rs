//! Bounded backpressure between asynchronous stdin and the filesystem worker.
use super::archive;
use crate::linux::{
    Error, Result,
    workspace::handoff::project::{DEADLINE, archive::open_project, lower_priority},
};
use agent_threads::managed::workspace::handoff::Receipt;
use std::{
    fs::File,
    io::{self, Cursor, Read},
    path::PathBuf,
    time::Instant,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::mpsc::{self, Receiver, Sender},
};

const CHUNK: usize = 128 * 1024;

pub(super) async fn unpack(
    input: impl AsyncRead + Unpin,
    home: PathBuf,
    binding: (Receipt, u64),
    locks: (File, File),
) -> Result<()> {
    let (receipt, quota) = binding;
    let (tx, rx) = mpsc::channel(4);
    let bytes = receipt.bytes;
    let worker = tokio::task::spawn_blocking(move || {
        let _locks = locks;
        lower_priority()?;
        let project = open_project(&home)?;
        let deadline = Instant::now() + DEADLINE;
        archive::unpack(
            Pipe {
                rx,
                current: Cursor::new(Vec::new()),
            },
            project,
            (&receipt, quota),
            deadline,
        )
    });
    tokio::time::timeout(DEADLINE, async {
        let (filled, restored) = tokio::join!(pump(input, tx, bytes), worker);
        restored.map_err(|_| Error::Refused("The checkpoint restore did not complete."))??;
        filled
    })
    .await
    .map_err(|_| Error::Refused("The checkpoint restore exceeded its deadline."))?
}

async fn pump(
    mut input: impl AsyncRead + Unpin,
    tx: Sender<Vec<u8>>,
    mut remaining: u64,
) -> Result<()> {
    while remaining != 0 {
        let count = usize::try_from(remaining.min(CHUNK as u64))
            .map_err(|_| Error::Refused("Invalid checkpoint chunk bound."))?;
        let mut chunk = vec![0; count];
        input.read_exact(&mut chunk).await.map_err(|_| {
            Error::Refused("The checkpoint stream ended before its declared length.")
        })?;
        tx.send(chunk)
            .await
            .map_err(|_| Error::Refused("The checkpoint reader closed."))?;
        remaining -= count as u64;
    }
    if input.read(&mut [0]).await? != 0 {
        return Err(Error::Refused("The checkpoint stream had trailing bytes."));
    }
    Ok(())
}

struct Pipe {
    rx: Receiver<Vec<u8>>,
    current: Cursor<Vec<u8>>,
}

impl Read for Pipe {
    fn read(&mut self, destination: &mut [u8]) -> io::Result<usize> {
        if destination.is_empty() {
            return Ok(0);
        }
        let got = Read::read(&mut self.current, destination)?;
        if got != 0 {
            return Ok(got);
        }
        let Some(next) = self.rx.blocking_recv() else {
            return Ok(0);
        };
        self.current = Cursor::new(next);
        Read::read(&mut self.current, destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_checkpoint_body_must_have_exact_length_and_end_at_eof() {
        let (tx, mut rx) = mpsc::channel(2);
        pump(b"project".as_slice(), tx, 7).await.unwrap();
        assert_eq!(rx.recv().await.unwrap(), b"project");
        assert!(rx.recv().await.is_none());
        for body in [b"short".as_slice(), b"project+".as_slice()] {
            let (tx, _rx) = mpsc::channel(2);
            assert!(pump(body, tx, 7).await.is_err());
        }
    }
}
