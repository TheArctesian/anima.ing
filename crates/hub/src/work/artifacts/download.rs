//! Signed-in download of the verified output of a completed source-build attempt.
mod publish;

use std::{io, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse as _, Response},
    routing::get,
};
use daemon_core::artifact;
use serde_json::json;
use sqlx::PgPool;

use super::{Error, Receipt, Store};
use crate::{
    clock::Clock,
    fleet::{
        access::{self, TokenKind},
        events::{self, ActorKind, AuditAction, AuditTarget, NewAudit, Operator},
    },
    ids::{DeviceId, JobId},
    work::jobs::{self, State as JobState},
};

#[derive(Clone)]
struct Downloads {
    db: PgPool,
    clock: Arc<dyn Clock>,
    store: Store,
}

pub(crate) fn router(db: PgPool, clock: Arc<dyn Clock>, store: Store) -> Router {
    Router::new()
        .route("/builds/v1/source-artifacts/{job}/{attempt}", get(download))
        .route(
            "/builds/v1/source-artifacts/{job}/{attempt}/image",
            get(image),
        )
        .merge(publish::router())
        .with_state(Downloads { db, clock, store })
}

async fn download(
    State(downloads): State<Downloads>,
    Path((job, attempt)): Path<(JobId, u32)>,
    request: Parts,
) -> Response {
    match authorized_download(&downloads, (job, attempt), &request).await {
        Ok(response) => response,
        Err(status) => status.into_response(),
    }
}

async fn authorized_download(
    downloads: &Downloads,
    (id, attempt): (JobId, u32),
    request: &Parts,
) -> Result<Response, StatusCode> {
    signed_in(downloads, request).await?;
    let client = access::client_addr(&request.extensions).ok_or(StatusCode::FORBIDDEN)?;
    let device = completed_attempt(downloads, id, attempt).await?;
    let (receipt, body) = downloads
        .store
        .read(device, id, attempt)
        .await
        .map_err(|error| read_status(&error))?;
    let operator = signed_in(downloads, request).await?;
    let mut tx = downloads
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    events::audit(
        &mut tx,
        downloads.clock.now(),
        NewAudit {
            operator,
            action: AuditAction::Download,
            target: AuditTarget::Job(id),
            client,
            change: json!({ "attempt": attempt, "receipt": receipt.id }),
        },
    )
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(response(&receipt, Body::from_stream(body)))
}

async fn image(
    State(downloads): State<Downloads>,
    Path((id, attempt)): Path<(JobId, u32)>,
    request: Parts,
) -> Result<Response, StatusCode> {
    signed_in(&downloads, &request).await?;
    access::client_addr(&request.extensions).ok_or(StatusCode::FORBIDDEN)?;
    let device = completed_attempt(&downloads, id, attempt).await?;
    let (receipt, image) = downloads
        .store
        .image((device, id, attempt))
        .await
        .map_err(|error| read_status(&error))?;
    signed_in(&downloads, &request).await?;
    let metadata = Json(json!({
        "job": id,
        "attempt": attempt,
        "receipt": receipt.id,
        "archiveDigest": receipt.artifact.archive_digest,
        "image": image,
    }));
    Ok(([(header::CACHE_CONTROL, "private, no-store")], metadata).into_response())
}

async fn completed_attempt(
    downloads: &Downloads,
    id: JobId,
    attempt: u32,
) -> Result<DeviceId, StatusCode> {
    let job = jobs::source_job(&downloads.db, id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .filter(|job| job.state == JobState::Done && job.attempt == attempt)
        .ok_or(StatusCode::NOT_FOUND)?;
    job.last_device.ok_or(StatusCode::NOT_FOUND)
}

fn read_status(error: &Error) -> StatusCode {
    match error {
        Error::Io(io::ErrorKind::NotFound) | Error::Conflict => StatusCode::NOT_FOUND,
        Error::Confirmation => StatusCode::CONFLICT,
        Error::Image => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn signed_in(downloads: &Downloads, request: &Parts) -> Result<Operator, StatusCode> {
    let identity = access::identify(&downloads.db, &request.headers, None, downloads.clock.now())
        .await
        .map_err(|error| {
            error.log();
            StatusCode::SERVICE_UNAVAILABLE
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    identity
        .operator()
        .filter(|operator| {
            operator.actor.kind() == ActorKind::User && identity.kind() == TokenKind::Login
        })
        .cloned()
        .ok_or(StatusCode::FORBIDDEN)
}

fn response(receipt: &Receipt, body: Body) -> Response {
    let mut response = body.into_response();
    let headers = response.headers_mut();
    for (name, value) in [
        (artifact::JOB, receipt.artifact.job.to_string()),
        (artifact::ATTEMPT, receipt.artifact.attempt.to_string()),
        (artifact::ARCHIVE, receipt.artifact.archive_digest.clone()),
        (artifact::MANIFEST, receipt.artifact.manifest_digest.clone()),
        (artifact::RECEIPT, receipt.id.to_string()),
        (
            header::CONTENT_LENGTH.as_str(),
            receipt.artifact.bytes.to_string(),
        ),
    ] {
        let Ok(value) = HeaderValue::from_str(&value) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        headers.insert(name, value);
    }
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"anima-source.oci\""),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}
