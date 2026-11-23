//! The hub's HTTP calls of publishing (BLD-064, BLD-066, D-28).
//! A response's length is never trusted: a read is refused at the first byte past the reported size.
//! Nothing here logs; a URL, a header or a response body never reaches an error (LOG-007).

use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};

use super::{
    bucket::{Signed, UploadUrl},
    files::Received,
};
use crate::fleet::builds::model::UploadedFile;

/// Enough of a write's response body to see an error inside a 200.
const MAX_REPLY_BYTES: usize = 64 * 1024;

/// The longest `ETag` the hub keeps; R2 sends a quoted MD5 or a multipart digest.
const MAX_ETAG_BYTES: usize = 128;

/// A try writes only while its hold has at least this long left (D-25).
pub(super) const WRITE_LIMIT: Duration = Duration::from_mins(2);

const UPLOAD_LIMIT: Duration = Duration::from_mins(30);

pub(super) enum Stop {
    Refused(String),
    /// The fault word is for the process log; the publisher tries again later (D-25).
    Unavailable(&'static str),
    /// A later try took the job, or this try's hold ran out: this try writes nothing more (D-25).
    TakenOver,
}

/// `kept` is the bytes kept for `SHA256SUMS`.
pub(super) struct Checked {
    pub(super) kept: Vec<u8>,
    pub(super) etag: String,
}

fn busy(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn request(http: &Client, signed: &Signed, body: Vec<u8>) -> RequestBuilder {
    let method = Method::from_bytes(signed.method.as_bytes()).unwrap_or(Method::GET);
    let mut request = http.request(method, &signed.url).body(body);
    for (name, value) in &signed.headers {
        request = request.header(name, value);
    }
    request
}

/// `tap` gets each chunk once it is counted, so it never sees a byte past the reported size (BLD-064, D-28).
pub(super) async fn read(
    http: &Client,
    signed: Signed,
    file: &UploadedFile,
    tap: &mut (dyn FnMut(&[u8]) + Send),
) -> Result<Checked, Stop> {
    let mut response = request(http, &signed, Vec::new())
        .send()
        .await
        .map_err(|_| Stop::Unavailable("bucket read"))?;
    let status = response.status();
    if busy(status) {
        return Err(Stop::Unavailable("bucket read"));
    }
    if status != StatusCode::OK {
        return Err(Stop::Refused(format!(
            "{} answered {} in the release bucket, not 200.",
            file.name,
            status.as_u16()
        )));
    }
    let etag = etag_of(&response).ok_or_else(|| {
        Stop::Refused(format!(
            "The release bucket gave no ETag for {}.",
            file.name
        ))
    })?;
    let mut received = Received::new(file);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Stop::Unavailable("bucket read"))?
    {
        received.push(&chunk).map_err(Stop::Refused)?;
        tap(&chunk);
    }
    let kept = received.finish().map_err(Stop::Refused)?;
    Ok(Checked { kept, etag })
}

pub(super) async fn read_small(
    http: &Client,
    signed: Signed,
    what: &str,
    max: usize,
) -> Result<Vec<u8>, Stop> {
    let mut response = request(http, &signed, Vec::new())
        .send()
        .await
        .map_err(|_| Stop::Unavailable("bucket read"))?;
    let status = response.status();
    if busy(status) {
        return Err(Stop::Unavailable("bucket read"));
    }
    if status != StatusCode::OK {
        return Err(Stop::Refused(format!(
            "{what} answered {} in the release bucket, not 200.",
            status.as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Stop::Unavailable("bucket read"))?
    {
        if bytes.len() + chunk.len() > max {
            return Err(Stop::Refused(format!(
                "{what} is longer than {max} bytes in the release bucket."
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn etag_of(response: &Response) -> Option<String> {
    let value = response
        .headers()
        .get(reqwest::header::ETAG)?
        .to_str()
        .ok()?;
    let fits = !value.is_empty() && value.len() <= MAX_ETAG_BYTES;
    fits.then(|| value.to_owned())
}

/// The bytes are not read: a cache may still hold an older copy, so they prove nothing (BLD-064, D-28).
pub(super) async fn readable(http: &Client, url: &str, name: &str) -> Result<(), Stop> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|_| Stop::Unavailable("public read"))?;
    let status = response.status();
    if busy(status) {
        return Err(Stop::Unavailable("public read"));
    }
    // Defence in depth: the client follows no redirect, and a redirect is still refused here.
    if status != StatusCode::OK || response.url().as_str() != url {
        return Err(Stop::Refused(format!(
            "{name} answered {} at the public address, not 200.",
            status.as_u16()
        )));
    }
    Ok(())
}

/// A 2xx whose body is an S3 `<Error>` (as `CopyObject` may send) is unavailable; a 412 means the copy source changed (D-28).
pub(super) async fn send(
    http: &Client,
    signed: Signed,
    body: Vec<u8>,
    what: &str,
) -> Result<(), Stop> {
    let mut response = request(http, &signed, body)
        .timeout(WRITE_LIMIT)
        .send()
        .await
        .map_err(|_| Stop::Unavailable("bucket write"))?;
    let status = response.status();
    if busy(status) {
        return Err(Stop::Unavailable("bucket write"));
    }
    if status == StatusCode::PRECONDITION_FAILED {
        return Err(Stop::Refused(format!(
            "The file behind {what} changed in the release bucket after the hub checked it."
        )));
    }
    if !status.is_success() {
        return Err(Stop::Refused(format!(
            "The release bucket refused the write of {what} ({}).",
            status.as_u16()
        )));
    }
    let mut reply = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Stop::Unavailable("bucket write"))?
    {
        reply.extend_from_slice(&chunk[..chunk.len().min(MAX_REPLY_BYTES - reply.len())]);
        if reply.len() >= MAX_REPLY_BYTES {
            break;
        }
    }
    if reply.windows(7).any(|window| window == b"<Error>") {
        return Err(Stop::Unavailable("bucket write"));
    }
    Ok(())
}

pub(super) async fn etag_now(http: &Client, signed: &Signed) -> Result<Option<String>, Stop> {
    let response = request(http, signed, Vec::new())
        .timeout(WRITE_LIMIT)
        .send()
        .await
        .map_err(|_| Stop::Unavailable("bucket head"))?;
    let status = response.status();
    if busy(status) {
        return Err(Stop::Unavailable("bucket head"));
    }
    Ok((status == StatusCode::OK)
        .then(|| etag_of(&response))
        .flatten())
}

/// The caller reads the object back afterwards: this answer alone proves nothing (BLD-064).
pub(super) async fn upload(
    http: &Client,
    upload: UploadUrl,
    body: bytes::Bytes,
) -> Result<(), Stop> {
    let mut request = http.put(&upload.url).timeout(UPLOAD_LIMIT).body(body);
    for (name, value) in &upload.headers {
        request = request.header(name, value);
    }
    let response = request
        .send()
        .await
        .map_err(|_| Stop::Unavailable("bucket write"))?;
    let status = response.status();
    if busy(status) {
        return Err(Stop::Unavailable("bucket write"));
    }
    if !status.is_success() {
        return Err(Stop::Refused(format!(
            "The release bucket refused the upload of {} ({}).",
            upload.name,
            status.as_u16()
        )));
    }
    Ok(())
}
