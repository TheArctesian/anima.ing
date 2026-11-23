//! The release bucket (BLD-060 to BLD-062, D-24, D-28). The R2 token is held only as `Zeroizing` (R-117).
//! The bucket has no `Debug`, and no error or log names a token, a key, a signature or a presigned URL (LOG-007).

use std::path::Path;

use chrono::{DateTime, TimeDelta, Utc};
use zeroize::Zeroizing;

use super::{
    sigv4::{self, Key, Request},
    updater_key::UpdaterKey,
};
use crate::{config::Releases, fleet::builds::model::UploadedFile};

/// How long a presigned upload URL works (BLD-060, D-28).
pub(super) const UPLOAD_URL_LIFE: TimeDelta = TimeDelta::minutes(15);

/// The `Cache-Control` of a versioned file (Section 3.10.4).
pub(super) const VERSIONED_CACHE: &str = "public, max-age=3600";

/// The `Cache-Control` of a stable name and of `latest.json` (BLD-066).
pub(super) const NO_CACHE: &str = "no-cache";

/// R2 takes any region name and documents `auto`.
const REGION: &str = "auto";
const SERVICE: &str = "s3";

const MAX_TOKEN_FILE_BYTES: usize = 1024;

/// Names the setting, never a value (SEC-004).
#[derive(Debug, thiserror::Error)]
pub enum BucketError {
    #[error("the R2 access key ID file cannot be read ({0})")]
    AccessKeyId(std::io::ErrorKind),
    #[error("the R2 secret access key file cannot be read ({0})")]
    SecretAccessKey(std::io::ErrorKind),
}

/// The `PUT` sends `headers` as given. No `Debug`: the URL is a credential for its object (LOG-007).
pub struct UploadUrl {
    pub name: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub expires_at: DateTime<Utc>,
}

pub(super) struct Write<'a> {
    pub(super) object: &'a str,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: &'a [u8],
}

pub(super) struct Signed {
    pub(super) method: &'static str,
    pub(super) url: String,
    pub(super) headers: Vec<(String, String)>,
}

pub struct Bucket {
    /// `scheme://host[:port]`.
    endpoint: String,
    name: String,
    public_base: String,
    access_key_id: Zeroizing<String>,
    secret: Zeroizing<String>,
    /// `None` for a bucket that only publishes what a workflow job signed (R-179).
    updater_key: Option<UpdaterKey>,
}

/// The buffer holds the most the file may have, so it never grows and leaves no copy behind.
fn token(path: &Path) -> std::io::Result<Zeroizing<String>> {
    use std::io::Read as _;
    let mut text = Zeroizing::new(String::with_capacity(MAX_TOKEN_FILE_BYTES + 1));
    std::fs::File::open(path)?
        .take(MAX_TOKEN_FILE_BYTES as u64)
        .read_to_string(&mut text)?;
    let trimmed = Zeroizing::new(text.trim().to_owned());
    if trimmed.is_empty() {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(trimmed)
}

impl Bucket {
    /// # Errors
    /// As [`Bucket::without_key`].
    pub fn open(releases: &Releases, updater_key: UpdaterKey) -> Result<Self, BucketError> {
        let mut bucket = Self::without_key(releases)?;
        bucket.updater_key = Some(updater_key);
        Ok(bucket)
    }

    /// Signs nothing; takes only files whose signature a release workflow job made (R-179).
    ///
    /// # Errors
    /// A token file that cannot be read or is empty.
    pub fn without_key(releases: &Releases) -> Result<Self, BucketError> {
        let access_key_id =
            token(&releases.access_key_id_file).map_err(|e| BucketError::AccessKeyId(e.kind()))?;
        let secret = token(&releases.secret_access_key_file)
            .map_err(|e| BucketError::SecretAccessKey(e.kind()))?;
        Ok(Self {
            endpoint: releases.endpoint.clone(),
            name: releases.bucket.clone(),
            public_base: releases.public_base.clone(),
            access_key_id,
            secret,
            updater_key: None,
        })
    }

    pub(super) fn public_base(&self) -> &str {
        &self.public_base
    }

    pub(super) const fn updater_key(&self) -> Option<&UpdaterKey> {
        self.updater_key.as_ref()
    }

    pub(super) fn public_url(&self, object: &str) -> String {
        format!("{}/{}", self.public_base, sigv4::encode(object, true))
    }

    fn key(&self) -> Key<'_> {
        Key {
            id: &self.access_key_id,
            secret: &self.secret,
            region: REGION,
            service: SERVICE,
        }
    }

    fn host(&self) -> &str {
        self.endpoint
            .split_once("://")
            .map_or(self.endpoint.as_str(), |(_, host)| host)
    }

    fn path(&self, object: &str) -> String {
        format!("/{}/{object}", self.name)
    }

    /// The signature binds the size, `Cache-Control` and type; the URL names one object (BLD-060, D-28).
    pub(super) fn upload_url(
        &self,
        folder: &str,
        (file, content_type): (&UploadedFile, &str),
        now: DateTime<Utc>,
    ) -> UploadUrl {
        let headers = vec![
            ("content-length".to_owned(), file.size.to_string()),
            ("cache-control".to_owned(), VERSIONED_CACHE.to_owned()),
            ("content-type".to_owned(), content_type.to_owned()),
        ];
        let path = self.path(&format!("{folder}/{}", file.name));
        let request = Request {
            method: "PUT",
            host: self.host(),
            path: &path,
            query: Vec::new(),
            headers: headers.clone(),
        };
        let life = u32::try_from(UPLOAD_URL_LIFE.num_seconds()).unwrap_or(u32::MAX);
        let query = sigv4::presign(&self.key(), request, now, life);
        UploadUrl {
            name: file.name.clone(),
            url: format!("{}{}?{query}", self.endpoint, sigv4::encode(&path, true)),
            headers,
            expires_at: now + UPLOAD_URL_LIFE,
        }
    }

    fn signed(&self, method: &'static str, write: Write<'_>, now: DateTime<Utc>) -> Signed {
        let path = self.path(write.object);
        let request = Request {
            method,
            host: self.host(),
            path: &path,
            query: Vec::new(),
            headers: write.headers,
        };
        let payload = sigv4::sha256_hex(write.body);
        Signed {
            method,
            url: format!("{}{}", self.endpoint, sigv4::encode(&path, true)),
            headers: sigv4::signed_headers(&self.key(), request, &payload, now),
        }
    }

    pub(super) fn put(&self, write: Write<'_>, now: DateTime<Utc>) -> Signed {
        self.signed("PUT", write, now)
    }

    /// From the bucket itself, past any cache (BLD-064, D-28).
    pub(super) fn get(&self, object: &str, now: DateTime<Utc>) -> Signed {
        let write = Write {
            object,
            headers: Vec::new(),
            body: b"",
        };
        self.signed("GET", write, now)
    }

    /// From the bucket itself, for its current `ETag` (D-28).
    pub(super) fn head(&self, object: &str, now: DateTime<Utc>) -> Signed {
        let write = Write {
            object,
            headers: Vec::new(),
            body: b"",
        };
        self.signed("HEAD", write, now)
    }

    pub(super) fn delete(&self, object: &str, now: DateTime<Utc>) -> Signed {
        let write = Write {
            object,
            headers: Vec::new(),
            body: b"",
        };
        self.signed("DELETE", write, now)
    }

    /// Copies only while `from` still has the `ETag` the hub checked (BLD-066, D-28).
    pub(super) fn copy(&self, (from, etag): (&str, &str), to: &str, now: DateTime<Utc>) -> Signed {
        let source = sigv4::encode(&self.path(from), true);
        let headers = vec![
            ("x-amz-copy-source".to_owned(), source),
            ("x-amz-copy-source-if-match".to_owned(), etag.to_owned()),
            ("x-amz-metadata-directive".to_owned(), "REPLACE".to_owned()),
            ("cache-control".to_owned(), NO_CACHE.to_owned()),
        ];
        let write = Write {
            object: to,
            headers,
            body: b"",
        };
        self.put(write, now)
    }
}
