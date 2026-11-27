//! Reauthorize the retained attempt before permitting a private registry write.
use axum::{extract::DefaultBodyLimit, routing::post};
use serde::Deserialize;
use uuid::Uuid;

use super::*;

pub(super) fn router() -> Router<Downloads> {
    Router::new()
        .route(
            "/builds/v1/source-artifacts/{job}/{attempt}/publish",
            post(publish),
        )
        .layer(DefaultBodyLimit::max(1024))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirmation {
    receipt: Uuid,
    image_digest: String,
}

async fn publish(
    State(downloads): State<Downloads>,
    Path((job, attempt)): Path<(JobId, u32)>,
    request: Parts,
    Json(confirmation): Json<Confirmation>,
) -> Result<Response, StatusCode> {
    signed_in(&downloads, &request).await?;
    let client = access::client_addr(&request.extensions).ok_or(StatusCode::FORBIDDEN)?;
    let device = completed_attempt(&downloads, job, attempt).await?;
    let ready = downloads
        .store
        .prepare_publication(
            (device, job, attempt),
            (confirmation.receipt, confirmation.image_digest.clone()),
        )
        .await
        .map_err(|error| read_status(&error))?
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let reference = ready.reference();
    let operator = signed_in(&downloads, &request).await?;
    if completed_attempt(&downloads, job, attempt).await? != device {
        return Err(StatusCode::NOT_FOUND);
    }
    let change = json!({
        "operation": "publish_source_image",
        "attempt": attempt,
        "receipt": confirmation.receipt,
        "imageDigest": confirmation.image_digest,
        "reference": reference,
    });
    audit(
        &downloads,
        (operator.clone(), client, job),
        AuditAction::Start,
        change.clone(),
    )
    .await?;
    ready.push().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    audit(
        &downloads,
        (operator, client, job),
        AuditAction::End,
        change,
    )
    .await?;
    let result = Json(json!({
        "job": job,
        "attempt": attempt,
        "receipt": confirmation.receipt,
        "imageDigest": confirmation.image_digest,
        "reference": reference,
    }));
    Ok(([(header::CACHE_CONTROL, "private, no-store")], result).into_response())
}

async fn audit(
    downloads: &Downloads,
    (operator, client, job): (Operator, access::ClientAddr, JobId),
    action: AuditAction,
    change: serde_json::Value,
) -> Result<(), StatusCode> {
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
            action,
            target: AuditTarget::Job(job),
            client,
            change,
        },
    )
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    tx.commit()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
