//! An upload in two passes (FIL-001, FIL-002, FIL-005): each part's additional data holds the
//! hash, known only at the end of the body, so the body is sealed into a staging copy first.
//! No pass holds more than two parts, and no plain byte is written to a disk.

use std::{io, path::PathBuf, sync::Arc};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt as _, stream};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use tokio::{fs, io::AsyncWriteExt as _};

use super::{
    Error, Store,
    location::{Cleaner, Position},
    model::{Attachment, MAX_FILE},
    parts::{Records, SealedStream, Sealer, damaged},
    pieces::{file_stream, private_folder, temporary_name},
    store,
    stream::Binding,
};
use crate::{engine::sealing_keys::SealingKey, ids::ArchiveId};

pub(super) struct Staged {
    path: PathBuf,
    binding: String,
    key: Arc<SealingKey>,
    pub(super) hash: String,
    pub(super) size: u64,
    pub(super) position: Option<Position>,
}

impl Drop for Staged {
    fn drop(&mut self) {
        // A copy left behind is sealed bytes that the next start of the folder does not read.
        if let Err(error) = std::fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(kind = %error.kind(), "a staging copy was not removed");
        }
    }
}

struct Reading {
    hasher: Sha256,
    size: u64,
    location: Option<Cleaner>,
    position: Option<Position>,
}

impl Reading {
    fn take(&mut self, chunk: &[u8]) -> Result<(), Error> {
        self.size += chunk.len() as u64;
        if self.size > MAX_FILE {
            return Err(Error::TooLarge);
        }
        self.hasher.update(chunk);
        if let Some(cleaner) = self.location.as_mut() {
            let walked = cleaner.feed(chunk);
            self.position = self.position.or(cleaner.position());
            if walked.is_err() || cleaner.passed() {
                self.location = None;
            }
        }
        Ok(())
    }
}

/// A body over [`MAX_FILE`] stops at the first byte past it with `TooLarge` (FIL-005).
pub(super) async fn stage<S, E>(
    store: &Store,
    key: SealingKey,
    body: S,
    keep_location: bool,
) -> Result<Staged, Error>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    let folder = store.staging.as_deref().ok_or(Error::Unavailable)?;
    private_folder(folder).await.map_err(staging)?;
    let mut marker = [0_u8; 16];
    getrandom::fill(&mut marker).map_err(|_| Error::Staging(io::ErrorKind::Other))?;
    let binding = super::hex(&marker);
    let mut staged = Staged {
        path: temporary_name(folder, "upload").map_err(staging)?,
        binding: format!("staging:{binding}"),
        key: Arc::new(key),
        hash: String::new(),
        size: 0,
        position: None,
    };
    let reading = write_staging(&staged, body, keep_location).await?;
    staged.hash = super::hex(&reading.hasher.finalize());
    staged.size = reading.size;
    staged.position = reading.position;
    Ok(staged)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "it is the function of `map_err`, which gives the error by value"
)]
fn staging(error: io::Error) -> Error {
    Error::Staging(error.kind())
}

async fn write_staging<S, E>(
    staged: &Staged,
    mut body: S,
    keep_location: bool,
) -> Result<Reading, Error>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    let binding = Binding {
        key: &staged.key,
        piece: &staged.binding,
    };
    let mut file = super::pieces::new_file(&staged.path)
        .await
        .map_err(staging)?;
    file.write_all(&binding.header()).await.map_err(staging)?;
    let mut reading = Reading {
        hasher: Sha256::new(),
        size: 0,
        location: keep_location.then(Cleaner::new),
        position: None,
    };
    let mut sealer = Sealer::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| Error::Body)?;
        reading.take(&chunk)?;
        let stored = sealer.push(binding, &chunk);
        file.write_all(&stored).await.map_err(staging)?;
    }
    file.write_all(&sealer.finish(binding))
        .await
        .map_err(staging)?;
    file.sync_all().await.map_err(staging)?;
    Ok(reading)
}

struct Resealing {
    records: Records,
    key: Arc<SealingKey>,
    staging: String,
    hash: String,
    number: u64,
    started: bool,
    done: bool,
}

async fn resealed(staged: &Staged) -> Result<SealedStream, Error> {
    let file = fs::File::open(&staged.path).await.map_err(staging)?;
    let state = Resealing {
        records: Records::new(file_stream(file)),
        key: staged.key.clone(),
        staging: staged.binding.clone(),
        hash: staged.hash.clone(),
        number: 0,
        started: false,
        done: false,
    };
    Ok(Box::pin(stream::try_unfold(
        state,
        |mut state| async move {
            if state.done {
                return Ok::<_, io::Error>(None);
            }
            let from = Binding {
                key: &state.key,
                piece: &state.staging,
            };
            let to = Binding {
                key: &state.key,
                piece: &state.hash,
            };
            let mut out = Vec::new();
            if !state.started {
                state.records.header(from).await?;
                out = to.header();
                state.started = true;
            }
            let Some((record, last)) = state.records.next().await? else {
                return Err(damaged(super::StreamError::Damaged));
            };
            let plain = from
                .open_part(state.number, last, &record)
                .map_err(damaged)?;
            out.extend(to.seal_part(state.number, last, &plain));
            state.number += 1;
            state.done = last;
            Ok(Some((Bytes::from(out), state)))
        },
    )))
}

pub(super) struct Placing<'a> {
    pub(super) owner: crate::engine::records::Reference,
    pub(super) name: String,
    pub(super) media_type: String,
    pub(super) archive: Option<ArchiveId>,
    pub(super) by: &'a str,
}

/// The piece stays locked until the caller's transaction ends, so a delete of it waits.
pub(super) async fn place(
    conn: &mut PgConnection,
    store: &Store,
    staged: &Staged,
    (placing, at): (Placing<'_>, DateTime<Utc>),
) -> Result<Attachment, Error> {
    let pieces = store.pieces()?;
    let key_name = staged.key.name();
    let piece = store::piece_key(key_name, &staged.hash);
    store::lock_piece(conn, &piece).await?;
    let stored = store::piece_exists(conn, &piece).await?
        && pieces.get(key_name, &staged.hash).await.is_ok();
    if !stored {
        let sealed_size = pieces
            .put(key_name, &staged.hash, resealed(staged).await?)
            .await?;
        store::insert_piece(
            conn,
            (key_name, &staged.hash),
            (staged.size, sealed_size),
            at,
        )
        .await?;
    }
    let attachment = Attachment {
        id: uuid::Uuid::now_v7().into(),
        owner: placing.owner,
        name: placing.name,
        media_type: placing.media_type,
        hash: staged.hash.clone(),
        key_name: key_name.to_owned(),
        size: staged.size,
        archive: placing.archive,
        position: staged.position,
        created_at: at,
    };
    store::insert_attachment(conn, &attachment, placing.by).await?;
    Ok(attachment)
}
