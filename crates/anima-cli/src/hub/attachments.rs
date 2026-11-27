//! File bytes outside GraphQL (K-E4 Section 9, FIL-005): no transfer holds the whole file, no error a byte of it.

use std::{fmt::Write as _, time::Duration};

use futures_util::StreamExt as _;
use http_body::Frame;
use http_body_util::StreamBody;
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use serde::Serialize;
use tokio::{fs::File, io::AsyncWriteExt as _};
use tokio_util::io::ReaderStream;

use super::{ACTOR_HEADER, AGENT, Hub, HubError, answer, bearer};

/// Long enough for a file of 256 MB over a slow link.
const TRANSFER_TIMEOUT: Duration = Duration::from_hours(1);
const MAX_REFUSAL: usize = 64 * 1024;
const LOCATION_REMOVED: &str = "anima-location-removed";

#[derive(Debug)]
pub struct Upload<'a> {
    pub owner: &'a str,
    pub name: &'a str,
    pub key: &'a str,
    pub archive: Option<&'a str>,
    pub keep_location: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloaded {
    pub size: u64,
    /// Whether the hub removed the location metadata from the copy (FIL-003).
    pub location_removed: bool,
}

impl Hub {
    pub(super) fn transfer_url(&self, path: &str) -> reqwest::Url {
        let mut url = self.origin().graphql();
        url.set_path(path);
        url
    }

    pub(super) fn transfer_request(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, HubError> {
        let mut request = request.timeout(TRANSFER_TIMEOUT);
        if let Some(token) = self.token() {
            request = request.header(AUTHORIZATION, bearer(token)?);
        }
        if self.declares_agent() {
            request = request.header(ACTOR_HEADER, AGENT);
        }
        Ok(request)
    }

    /// # Errors
    /// [`HubError::Refused`] (`VALIDATION`, or `UNAVAILABLE` with no file store); a failed request.
    pub async fn upload_attachment(
        &self,
        upload: &Upload<'_>,
        (file, length): (File, u64),
        media_type: &str,
    ) -> Result<serde_json::Value, HubError> {
        let query = query_text(upload)?;
        let mut url = self.transfer_url("/files/v1/attachments");
        url.set_query(Some(&query));
        let frames = ReaderStream::new(file).map(|chunk| chunk.map(Frame::data));
        let request = self
            .http
            .post(url)
            .header(CONTENT_TYPE, media_type)
            .header(CONTENT_LENGTH, length)
            .body(reqwest::Body::wrap(StreamBody::new(frames)));
        let response = self
            .transfer_request(request)?
            .send()
            .await
            .map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        let answer = self.read_small(response).await?;
        if status == 201 {
            return serde_json::from_slice(&answer).map_err(|_| HubError::Malformed { status });
        }
        Err(answer::refusal(status, &answer))
    }

    /// # Errors
    /// [`HubError::Refused`] (`NOT_FOUND`, `UNAVAILABLE`); a failed request or write.
    pub async fn download_attachment(
        &self,
        id: &str,
        to: &mut File,
    ) -> Result<Downloaded, HubError> {
        let url = self.transfer_url(&format!("/files/v1/attachments/{}", path_part(id)));
        let mut response = self
            .transfer_request(self.http.get(url))?
            .send()
            .await
            .map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        if status != 200 {
            let answer = self.read_small(response).await?;
            return Err(answer::refusal(status, &answer));
        }
        let location_removed = response
            .headers()
            .get(LOCATION_REMOVED)
            .is_some_and(|value| value.as_bytes() == b"true");
        let mut size = 0_u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.unreachable(&error))?
        {
            to.write_all(&chunk)
                .await
                .map_err(|error| HubError::Unreachable {
                    origin: self.origin().as_text(),
                    reason: format!("the file could not be written ({})", error.kind()),
                })?;
            size += chunk.len() as u64;
        }
        Ok(Downloaded {
            size,
            location_removed,
        })
    }

    async fn read_small(&self, mut response: reqwest::Response) -> Result<Vec<u8>, HubError> {
        let mut answer = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.unreachable(&error))?
        {
            if answer.len() + chunk.len() > MAX_REFUSAL {
                break;
            }
            answer.extend_from_slice(&chunk);
        }
        Ok(answer)
    }
}

fn query_text(upload: &Upload<'_>) -> Result<String, HubError> {
    let mut url = reqwest::Url::parse("http://query.invalid/").map_err(|_| HubError::Setup)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("owner", upload.owner);
        pairs.append_pair("name", upload.name);
        pairs.append_pair("key", upload.key);
        if let Some(archive) = upload.archive {
            pairs.append_pair("archive", archive);
        }
        if upload.keep_location {
            pairs.append_pair("keep_location", "true");
        }
    }
    Ok(url.query().unwrap_or_default().to_owned())
}

fn path_part(id: &str) -> String {
    let mut part = String::with_capacity(id.len());
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' {
            part.push(char::from(byte));
        } else {
            // Writing to a `String` cannot fail.
            let _ = write!(part, "%{byte:02X}");
        }
    }
    part
}
