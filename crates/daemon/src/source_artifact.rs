//! Verified local artifact consumption; a partial stdout stream is never delivery proof.
use bytes::Bytes;
use daemon_core::{
    artifact::{MAX_ARCHIVE_BYTES, Offer},
    source_jobs::{Action, MAX_REQUEST_BYTES, Request, SOCKET},
};
use futures_util::{Stream, stream};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::{
    io::{self, IsTerminal},
    process::ExitCode,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
const HEADER_LIMIT: u64 = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    ok: bool,
    value: Artifact,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Artifact {
    bytes: u64,
    archive_digest: String,
    manifest_digest: String,
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "The artifact transfer is incomplete or invalid; discard its output.",
    )
}

/// Runs the closed local export command, without hub credentials or remote effects.
pub async fn run() -> ExitCode {
    if std::io::stdout().is_terminal() {
        eprintln!("Redirect the artifact to a file or verified consumer.");
        return ExitCode::FAILURE;
    }
    if let Ok(Ok(())) = tokio::time::timeout(Duration::from_mins(30), stdio()).await {
        ExitCode::SUCCESS
    } else {
        eprintln!("Artifact transfer failed; discard the incomplete output.");
        ExitCode::FAILURE
    }
}

async fn stdio() -> io::Result<()> {
    let mut frame = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        BufReader::new(tokio::io::stdin())
            .take(MAX_REQUEST_BYTES as u64 + 1)
            .read_until(b'\n', &mut frame),
    )
    .await
    .map_err(|_| invalid())??;
    if frame.len() > MAX_REQUEST_BYTES || frame.last() != Some(&b'\n') {
        return Err(invalid());
    }
    let request: Request = serde_json::from_slice(&frame).map_err(|_| invalid())?;
    let Action::Export {
        ref archive_digest,
        ref manifest_digest,
        ..
    } = request.action
    else {
        return Err(invalid());
    };
    let input = connect(&frame).await?;
    receive(
        input,
        &mut tokio::io::stdout(),
        archive_digest,
        manifest_digest,
    )
    .await
}

async fn connect(frame: &[u8]) -> io::Result<UnixStream> {
    let mut stream = UnixStream::connect(SOCKET).await?;
    if stream.peer_cred()?.uid() != 0 {
        return Err(invalid());
    }
    stream.write_all(frame).await?;
    Ok(stream)
}

pub(crate) async fn export(
    request: &Request,
    offer: &Offer,
) -> io::Result<impl Stream<Item = io::Result<Bytes>> + use<>> {
    let mut frame = serde_json::to_vec(request).map_err(|_| invalid())?;
    if frame.len() >= MAX_REQUEST_BYTES || !offer.valid() {
        return Err(invalid());
    }
    frame.push(b'\n');
    let input = connect(&frame).await?;
    let reader = verified(input, &offer.archive_digest, &offer.manifest_digest).await?;
    if reader.remaining != offer.bytes {
        return Err(invalid());
    }
    Ok(stream::try_unfold(reader, |mut reader| async move {
        Ok(reader.next().await?.map(|chunk| (chunk, reader)))
    }))
}

async fn receive(
    input: impl AsyncRead + Unpin,
    output: &mut (impl tokio::io::AsyncWrite + Unpin),
    archive: &str,
    manifest: &str,
) -> io::Result<()> {
    let mut reader = verified(input, archive, manifest).await?;
    while let Some(chunk) = reader.next().await? {
        output.write_all(&chunk).await?;
    }
    output.flush().await
}

async fn verified<R: AsyncRead + Unpin>(
    input: R,
    archive: &str,
    manifest: &str,
) -> io::Result<Verified<R>> {
    let mut reader = BufReader::new(input);
    let mut line = Vec::new();
    (&mut reader)
        .take(HEADER_LIMIT + 1)
        .read_until(b'\n', &mut line)
        .await?;
    if line.len() as u64 > HEADER_LIMIT || line.last() != Some(&b'\n') {
        return Err(invalid());
    }
    let header: Header = serde_json::from_slice(&line).map_err(|_| invalid())?;
    if !header.ok
        || !(1..=MAX_ARCHIVE_BYTES).contains(&header.value.bytes)
        || header.value.archive_digest != archive
        || header.value.manifest_digest != manifest
    {
        return Err(invalid());
    }
    Ok(Verified {
        reader,
        remaining: header.value.bytes,
        digest: header.value.archive_digest,
        hash: Sha256::new(),
    })
}

struct Verified<R> {
    reader: BufReader<R>,
    remaining: u64,
    digest: String,
    hash: Sha256,
}

impl<R: AsyncRead + Unpin> Verified<R> {
    async fn next(&mut self) -> io::Result<Option<Bytes>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        let wanted = usize::try_from(self.remaining.min(65536)).map_err(|_| invalid())?;
        let mut chunk = vec![0_u8; wanted];
        let count = self.reader.read(&mut chunk).await?;
        if count == 0 {
            return Err(invalid());
        }
        chunk.truncate(count);
        self.hash.update(&chunk);
        self.remaining -= count as u64;
        if self.remaining == 0 {
            self.check_end().await?;
        }
        Ok(Some(Bytes::from(chunk)))
    }

    async fn check_end(&mut self) -> io::Result<()> {
        let mut computed = String::from("sha256:");
        for byte in std::mem::take(&mut self.hash).finalize() {
            write!(&mut computed, "{byte:02x}").map_err(|_| invalid())?;
        }
        if self.reader.read(&mut [0_u8; 1]).await? != 0 || computed != self.digest {
            return Err(invalid());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
