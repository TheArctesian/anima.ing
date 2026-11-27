//! Source-build archive bytes and their private receipt outside GraphQL.
use std::fmt::Write as _;

use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncWriteExt as _};
use uuid::Uuid;

use super::{Code, Hub, HubError, answer};

mod publish;
pub use publish::PublishedImage;

const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceArtifact {
    pub job: Uuid,
    pub attempt: u32,
    pub receipt: Uuid,
    pub bytes: u64,
    pub archive_digest: String,
    pub manifest_digest: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceImage {
    pub job: Uuid,
    pub attempt: u32,
    pub receipt: Uuid,
    pub archive_digest: String,
    pub image: VerifiedImage,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedImage {
    pub digest: String,
    pub media_type: String,
    pub platforms: Vec<Platform>,
}

#[derive(Deserialize, Serialize)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
    pub variant: Option<String>,
}

impl Hub {
    /// Read the verified runnable image retained for a completed attempt.
    ///
    /// # Errors
    /// A refused request or malformed response.
    pub async fn inspect_source_image(
        &self,
        job: Uuid,
        attempt: u32,
    ) -> Result<SourceImage, HubError> {
        let path = format!("/builds/v1/source-artifacts/{job}/{attempt}/image");
        let url = self.transfer_url(&path);
        let response = self
            .transfer_request(self.http.get(url))?
            .send()
            .await
            .map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        match status {
            200 => (),
            404 => return Err(artifact_not_found()),
            422 => {
                return Err(HubError::Refused {
                    code: Code::Validation,
                    message: "The retained archive could not be verified as a runnable image."
                        .into(),
                    fields: Vec::new(),
                });
            }
            _ => return Err(answer::refusal(status, &[])),
        }
        let image: SourceImage = self.image_answer(response).await?;
        if image.job != job
            || image.attempt != attempt
            || image.receipt.is_nil()
            || !valid_digest(&image.archive_digest)
            || !valid_digest(&image.image.digest)
            || image.image.platforms.is_empty()
        {
            return Err(HubError::Malformed { status });
        }
        Ok(image)
    }

    async fn image_answer<T: DeserializeOwned>(
        &self,
        mut response: reqwest::Response,
    ) -> Result<T, HubError> {
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.unreachable(&error))?
        {
            if chunk.len() > 64 * 1024 - bytes.len() {
                return Err(HubError::Malformed { status });
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| HubError::Malformed { status })
    }

    /// Streams one archive to a temporary file and checks its length and SHA-256.
    ///
    /// # Errors
    /// A refused request, invalid receipt or archive, or failed write.
    pub async fn download_source_artifact(
        &self,
        job: Uuid,
        attempt: u32,
        to: &mut File,
    ) -> Result<SourceArtifact, HubError> {
        let path = format!("/builds/v1/source-artifacts/{job}/{attempt}");
        let url = self.transfer_url(&path);
        let mut response = self
            .transfer_request(self.http.get(url))?
            .send()
            .await
            .map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        if status == 404 {
            return Err(artifact_not_found());
        }
        if status != 200 {
            return Err(answer::refusal(status, &[]));
        }
        let invalid = HubError::Malformed { status };
        let receipt =
            SourceArtifact::from_headers(response.headers(), (job, attempt)).ok_or(invalid)?;
        let mut hasher = Sha256::new();
        let mut remaining = receipt.bytes;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.unreachable(&error))?
        {
            remaining = remaining
                .checked_sub(chunk.len() as u64)
                .ok_or(HubError::Malformed { status })?;
            hasher.update(&chunk);
            to.write_all(&chunk)
                .await
                .map_err(|error| HubError::Unreachable {
                    origin: self.origin().as_text(),
                    reason: format!("the archive could not be written ({})", error.kind()),
                })?;
        }
        if remaining != 0 || digest_text(&hasher.finalize()) != receipt.archive_digest {
            return Err(HubError::Malformed { status });
        }
        Ok(receipt)
    }
}

impl SourceArtifact {
    fn from_headers(headers: &HeaderMap, (job, attempt): (Uuid, u32)) -> Option<Self> {
        let one = |name: &str| -> Option<&str> {
            let values = headers.get_all(name);
            let mut values = values.iter();
            let value = values.next()?.to_str().ok()?;
            values.next().is_none().then_some(value)
        };
        if one("anima-artifact-job")?.parse::<Uuid>().ok()? != job
            || one("anima-artifact-attempt")?.parse::<u32>().ok()? != attempt
        {
            return None;
        }
        let receipt: Uuid = one("anima-artifact-receipt")?.parse().ok()?;
        let bytes = one("content-length")?.parse().ok()?;
        let archive_digest = one("anima-artifact-archive")?;
        let manifest_digest = one("anima-artifact-manifest")?;
        if receipt.is_nil()
            || !(1..=MAX_ARCHIVE_BYTES).contains(&bytes)
            || !valid_digest(archive_digest)
            || !valid_digest(manifest_digest)
        {
            return None;
        }
        Some(Self {
            job,
            attempt,
            receipt,
            bytes,
            archive_digest: archive_digest.to_owned(),
            manifest_digest: manifest_digest.to_owned(),
        })
    }
}

fn artifact_not_found() -> HubError {
    refused(
        Code::NotFound,
        "No retained artifact exists for that completed build attempt.",
    )
}

fn refused(code: Code, message: &str) -> HubError {
    HubError::Refused {
        code,
        message: message.into(),
        fields: Vec::new(),
    }
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn digest_text(bytes: &[u8]) -> String {
    let mut text = String::from("sha256:");
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}
