//! Publish one exact retained image through the signed-in hub session.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Code, Hub, HubError, answer, artifact_not_found, refused, valid_digest};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedImage {
    pub job: Uuid,
    pub attempt: u32,
    pub receipt: Uuid,
    pub image_digest: String,
    pub reference: String,
}

impl Hub {
    /// Publish the explicitly confirmed image to the hub's private registry.
    ///
    /// # Errors
    /// A refused request, registry verification failure or malformed response.
    pub async fn publish_source_image(
        &self,
        (job, attempt): (Uuid, u32),
        receipt: Uuid,
        digest: &str,
    ) -> Result<PublishedImage, HubError> {
        if receipt.is_nil() || !valid_digest(digest) {
            return Err(refused(
                Code::Validation,
                "Supply the receipt UUID and SHA-256 image digest from inspect-image.",
            ));
        }
        let path = format!("/builds/v1/source-artifacts/{job}/{attempt}/publish");
        let input = serde_json::json!({"receipt": receipt, "imageDigest": digest});
        let request = self.transfer_request(self.http.post(self.transfer_url(&path)))?;
        let response = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(input.to_string())
            .send()
            .await
            .map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        match status {
            200 => (),
            404 => return Err(artifact_not_found()),
            409 => {
                return Err(refused(
                    Code::Conflict,
                    "The receipt or image digest has changed. Inspect the build image again.",
                ));
            }
            422 => {
                return Err(refused(
                    Code::Validation,
                    "The retained image could not be verified for publication.",
                ));
            }
            502 => {
                return Err(refused(
                    Code::Unavailable,
                    "Registry confirmation is pending. Retry with the same receipt and image digest.",
                ));
            }
            503 => {
                return Err(refused(
                    Code::Unavailable,
                    "Private registry publication is unavailable on this hub.",
                ));
            }
            _ => return Err(answer::refusal(status, &[])),
        }
        let published: PublishedImage = self.image_answer(response).await?;
        if published.job != job
            || published.attempt != attempt
            || published.receipt != receipt
            || published.image_digest != digest
            || !valid_reference(&published.reference, digest)
        {
            return Err(HubError::Malformed { status });
        }
        Ok(published)
    }
}

fn valid_reference(value: &str, digest: &str) -> bool {
    value.split_once('@').is_some_and(|(repository, hash)| {
        let Some((host, path)) = repository.split_once('/') else {
            return false;
        };
        hash == digest
            && (host == "localhost" || host.contains(['.', ':']))
            && host.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'-' | b'[' | b']')
            })
            && path.split('/').all(|part| {
                !part.is_empty()
                    && part.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'_' | b'-')
                    })
            })
    })
}
