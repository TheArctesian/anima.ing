//! Immutable OCI Distribution writes and digest readback.
use std::{fs::File, io, os::unix::fs::FileExt as _, sync::Arc, time::Duration};

use oci_artifact::Part;
use reqwest::{Body, Client, Response, StatusCode, Url, header};
use sha2::{Digest as _, Sha256};

use super::{Destination, Error};
use crate::work::artifacts::model::encoded_digest;

const DIGEST: &str = "docker-content-digest";

pub(super) struct Transport {
    client: Client,
    destination: Destination,
}

impl Transport {
    pub(super) const fn new(client: Client, destination: Destination) -> Self {
        Self {
            client,
            destination,
        }
    }

    pub(super) fn reference(&self, digest: &str) -> String {
        self.destination.reference(digest)
    }

    pub(super) async fn push(&self, file: &Arc<File>, part: &Part) -> Result<(), Error> {
        match part.media_type.as_deref() {
            Some(media) => self.manifest(file, part, media).await,
            None => self.blob(file, part).await,
        }
    }

    async fn blob(&self, file: &Arc<File>, part: &Part) -> Result<(), Error> {
        let blob = self.destination.object(&format!("blobs/{}", part.digest))?;
        if self.blob_present(blob.clone(), part).await? {
            return Ok(());
        }
        let opened = self
            .client
            .post(self.destination.object("blobs/uploads/")?)
            .header(header::CONTENT_LENGTH, 0)
            .send()
            .await
            .map_err(|_| Error::Registry)?;
        if opened.status() != StatusCode::ACCEPTED {
            return Err(Error::Registry);
        }
        let location = opened
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(Error::Registry)?;
        let session = self.destination.upload(location)?;
        let mut upload = session.clone();
        upload.query_pairs_mut().append_pair("digest", &part.digest);
        if let Err(error) = self
            .write((upload, "application/octet-stream"), file, part)
            .await
        {
            let _ = self
                .client
                .delete(session)
                .timeout(Duration::from_secs(5))
                .send()
                .await;
            return Err(error);
        }
        self.blob_present(blob, part)
            .await?
            .then_some(())
            .ok_or(Error::Registry)
    }

    async fn blob_present(&self, url: Url, part: &Part) -> Result<bool, Error> {
        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|_| Error::Registry)?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(false),
            StatusCode::OK
                if digest_header(&response, part)
                    && response
                        .headers()
                        .get(header::CONTENT_LENGTH)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        == Some(part.size) =>
            {
                Ok(true)
            }
            _ => Err(Error::Registry),
        }
    }

    async fn manifest(&self, file: &Arc<File>, part: &Part, media: &str) -> Result<(), Error> {
        let url = self
            .destination
            .object(&format!("manifests/{}", part.digest))?;
        if self.manifest_present(url.clone(), part, media).await? {
            return Ok(());
        }
        self.write((url.clone(), media), file, part).await?;
        self.manifest_present(url, part, media)
            .await?
            .then_some(())
            .ok_or(Error::Registry)
    }

    async fn manifest_present(&self, url: Url, part: &Part, media: &str) -> Result<bool, Error> {
        let mut response = self
            .client
            .get(url)
            .header(header::ACCEPT, media)
            .send()
            .await
            .map_err(|_| Error::Registry)?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(false),
            StatusCode::OK if digest_header(&response, part) => (),
            _ => return Err(Error::Registry),
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim);
        if content_type != Some(media) {
            return Err(Error::Registry);
        }
        let mut seen = 0;
        let mut hash = Sha256::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Registry)? {
            seen += chunk.len() as u64;
            if seen > part.size {
                return Err(Error::Registry);
            }
            hash.update(chunk);
        }
        if seen != part.size || encoded_digest(&hash.finalize()) != part.digest {
            return Err(Error::Registry);
        }
        Ok(true)
    }

    async fn write(
        &self,
        (url, media): (Url, &str),
        file: &Arc<File>,
        part: &Part,
    ) -> Result<(), Error> {
        let response = self
            .client
            .put(url)
            .header(header::CONTENT_LENGTH, part.size)
            .header(header::CONTENT_TYPE, media)
            .body(range(file.clone(), part))
            .send()
            .await
            .map_err(|_| Error::Registry)?;
        if response.status() != StatusCode::CREATED || !digest_header(&response, part) {
            return Err(Error::Registry);
        }
        Ok(())
    }
}

fn digest_header(response: &Response, part: &Part) -> bool {
    response
        .headers()
        .get(DIGEST)
        .is_some_and(|value| value == &part.digest)
}

fn range(file: Arc<File>, part: &Part) -> Body {
    let input = futures_util::stream::try_unfold(
        (file, part.offset, part.size),
        |(file, offset, left)| async move {
            if left == 0 {
                return Ok::<_, io::Error>(None);
            }
            let count = usize::try_from(left.min(64 * 1024)).map_err(io::Error::other)?;
            let (file, chunk) = tokio::task::spawn_blocking(move || {
                let mut chunk = vec![0; count];
                file.read_exact_at(&mut chunk, offset)?;
                Ok::<_, io::Error>((file, bytes::Bytes::from(chunk)))
            })
            .await
            .map_err(io::Error::other)??;
            Ok(Some((
                chunk,
                (file, offset + count as u64, left - count as u64),
            )))
        },
    );
    Body::wrap_stream(input)
}
