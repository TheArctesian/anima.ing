use super::{Answer, Body, Cause, HubClient, answer, cause};
use daemon_core::{
    artifact::{self, Offer, Receipt},
    source_jobs::Request,
    wire::DaemonToken,
};
use std::time::Duration;

impl HubClient {
    pub(crate) async fn upload_artifact(
        &self,
        token: &DaemonToken,
        request: &Request,
        offer: &Offer,
    ) -> Answer<Receipt> {
        if self.local.is_some() || self.base.scheme() != "https" {
            return Answer::Failed(Cause::Unreadable);
        }
        tokio::time::timeout(
            Duration::from_mins(30),
            self.upload_source(token, request, offer),
        )
        .await
        .unwrap_or(Answer::Failed(Cause::Timeout))
    }

    async fn upload_source(
        &self,
        token: &DaemonToken,
        request: &Request,
        offer: &Offer,
    ) -> Answer<Receipt> {
        let Ok(url) = self.base.join(artifact::ROUTE) else {
            return Answer::Failed(Cause::Unreadable);
        };
        let Ok(stream) = crate::source_artifact::export(request, offer).await else {
            return Answer::Failed(Cause::Unreadable);
        };
        let sent = self
            .http
            .post(url)
            .timeout(Duration::from_mins(30))
            .bearer_auth(token.expose())
            .header("content-type", "application/octet-stream")
            .header(artifact::JOB, offer.job.to_string())
            .header(artifact::ATTEMPT, offer.attempt)
            .header(artifact::BYTES, offer.bytes)
            .header(artifact::ARCHIVE, &offer.archive_digest)
            .header(artifact::MANIFEST, &offer.manifest_digest)
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await;
        match sent {
            Ok(response) => read(response, request, offer).await,
            Err(error) => Answer::Failed(cause(&error)),
        }
    }
}

async fn read(
    mut response: reqwest::Response,
    request: &Request,
    offer: &Offer,
) -> Answer<Receipt> {
    let status = response.status().as_u16();
    let mut body = Body::new();
    loop {
        match response.chunk().await {
            Ok(None) => break,
            Ok(Some(chunk)) => {
                body.add(&chunk);
                if body.is_full() {
                    return Answer::Failed(Cause::Unreadable);
                }
            }
            Err(error) => return Answer::Failed(cause(&error)),
        }
    }
    answer(status, body.bytes(), 200, |body| {
        let receipt: Receipt = serde_json::from_slice(body).ok()?;
        (receipt.valid() && receipt.device == request.device && receipt.artifact == *offer)
            .then_some(receipt)
    })
}
