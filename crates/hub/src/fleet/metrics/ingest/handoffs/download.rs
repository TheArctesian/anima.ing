use std::{io, time::Duration};

use agent_threads::managed::workspace::handoff::Import;
use axum::{
    Extension,
    body::{Body, Bytes},
    extract::{Path, Request, State},
    http::header,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::RefusalCode;
use tokio::io::AsyncReadExt as _;
use uuid::Uuid;

use super::{Access, DaemonState, Stop, access::custody_error, busy, custody, storage_error};
use crate::{
    ids::DeviceId,
    work::artifacts::{Store, checkpoints::Open},
};

const CHUNK_BYTES: usize = 64 * 1024;
const RECHECK: TimeDelta = TimeDelta::seconds(5);

pub(super) async fn serve(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    let Some(Extension(store)) = store else {
        return busy();
    };
    let (access, _, _) = match Access::open(state, request).await {
        Ok(parts) => parts,
        Err(response) => return *response,
    };
    let result = response(&access, id, store).await;
    access.answer(result).await
}

async fn response(access: &Access, id: Uuid, store: Store) -> Result<Response, Stop> {
    let (device, import, _) = authorize(access, id).await?;
    let checkpoint = store
        .open_checkpoint(import.clone())
        .await
        .map_err(|error| storage_error(device, &error))?;
    let (holder, current, expires) = authorize(access, id).await?;
    if holder != device || current != import {
        return Err(Stop::Refused(RefusalCode::NotHolder, Some(device)));
    }
    let length = import.receipt.bytes;
    let download = Download {
        access: access.clone(),
        import,
        checkpoint,
        device,
        expires,
        remaining: length,
        recheck_at: access.now() + RECHECK,
    };
    let body = Body::from_stream(futures_util::stream::try_unfold(download, Download::chunk));
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
            (header::CONTENT_LENGTH, length.to_string()),
        ],
        body,
    )
        .into_response())
}

async fn authorize(access: &Access, id: Uuid) -> Result<(DeviceId, Import, DateTime<Utc>), Stop> {
    let mut guard = access.lock().await?;
    let device = guard.device;
    let transfer = custody::read(&mut guard.tx, device, id, access.now())
        .await
        .map_err(|error| custody_error(device, error))?;
    let expires = transfer.expires_at.min(guard.expires);
    let import = transfer
        .checkpoint
        .as_ref()
        .and_then(|receipt| transfer.import(&receipt.0))
        .filter(|_| access.now() < expires)
        .ok_or(Stop::Refused(RefusalCode::NotHolder, Some(device)))?;
    guard.commit().await?;
    Ok((device, import, expires))
}

struct Download {
    access: Access,
    import: Import,
    checkpoint: Open,
    device: DeviceId,
    expires: DateTime<Utc>,
    recheck_at: DateTime<Utc>,
    remaining: u64,
}

impl Download {
    async fn chunk(mut self) -> io::Result<Option<(Bytes, Self)>> {
        if self.access.now() >= self.expires {
            return Err(revoked());
        }
        if self.remaining == 0 {
            return Ok(None);
        }
        if self.access.now() >= self.recheck_at {
            self.recheck().await?;
        }
        let limit = usize::try_from(self.remaining)
            .unwrap_or(CHUNK_BYTES)
            .min(CHUNK_BYTES);
        let mut bytes = vec![0; limit];
        let read = tokio::time::timeout(
            Duration::from_secs(5),
            self.checkpoint.file.read(&mut bytes),
        )
        .await
        .map_err(io::Error::other)??;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if self.access.now() >= self.expires {
            return Err(revoked());
        }
        bytes.truncate(read);
        self.remaining -= u64::try_from(read).map_err(io::Error::other)?;
        Ok(Some((Bytes::from(bytes), self)))
    }

    async fn recheck(&mut self) -> io::Result<()> {
        let id = self.import.receipt.source.binding.transfer;
        let (device, current, expires) =
            authorize(&self.access, id).await.map_err(|_| revoked())?;
        if device != self.device || current != self.import {
            return Err(revoked());
        }
        self.expires = expires;
        self.recheck_at = self.access.now() + RECHECK;
        Ok(())
    }
}

fn revoked() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Workspace transfer is no longer available.",
    )
}
