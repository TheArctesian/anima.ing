//! The I/O edge to Hugging Face; no token is sent, as the library reads public repositories only.

use std::{future::Future, time::Duration};

use reqwest::{Client, StatusCode, Url};

const LONGEST_ANSWER: usize = 8 * 1024 * 1024;
/// The largest file the hub hashes itself; a larger file is in LFS, which gives the value.
pub const LONGEST_FILE: u64 = 16 * 1024 * 1024;

/// The messages hold no repository name (LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("the repository or the file does not exist")]
    NotFound,
    #[error("the source did not answer as expected")]
    Unavailable,
    #[error("the answer is larger than the hub reads")]
    TooLarge,
}

pub trait Fetch: Sync {
    /// The answer for `organisation/name`, with the size and LFS value of each file.
    fn repository(
        &self,
        repository: &str,
    ) -> impl Future<Output = Result<Vec<u8>, FetchError>> + Send;

    fn file(
        &self,
        repository: &str,
        revision: &str,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, FetchError>> + Send;
}

pub struct HuggingFace {
    client: Client,
    base: String,
}

impl HuggingFace {
    /// # Errors
    /// `Unavailable` when the HTTP client cannot be built.
    pub fn public() -> Result<Self, FetchError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| FetchError::Unavailable)?;
        Ok(Self {
            client,
            base: "https://huggingface.co".to_owned(),
        })
    }

    /// Each part is one path segment, so no part can add a path, a query or a fragment.
    fn url(&self, parts: &[&str]) -> Result<Url, FetchError> {
        let mut url = Url::parse(&self.base).map_err(|_| FetchError::Unavailable)?;
        url.path_segments_mut()
            .map_err(|()| FetchError::Unavailable)?
            .extend(parts);
        Ok(url)
    }

    async fn get(&self, url: Url, longest: usize) -> Result<Vec<u8>, FetchError> {
        let mut answer = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| FetchError::Unavailable)?;
        match answer.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND | StatusCode::UNAUTHORIZED => return Err(FetchError::NotFound),
            _ => return Err(FetchError::Unavailable),
        }
        let mut body = Vec::new();
        while let Some(chunk) = answer.chunk().await.map_err(|_| FetchError::Unavailable)? {
            body.extend_from_slice(&chunk);
            if body.len() > longest {
                return Err(FetchError::TooLarge);
            }
        }
        Ok(body)
    }
}

impl Fetch for HuggingFace {
    async fn repository(&self, repository: &str) -> Result<Vec<u8>, FetchError> {
        let (organisation, name) = repository.split_once('/').ok_or(FetchError::NotFound)?;
        let mut url = self.url(&["api", "models", organisation, name])?;
        url.set_query(Some("blobs=true"));
        self.get(url, LONGEST_ANSWER).await
    }

    async fn file(
        &self,
        repository: &str,
        revision: &str,
        path: &str,
    ) -> Result<Vec<u8>, FetchError> {
        let (organisation, name) = repository.split_once('/').ok_or(FetchError::NotFound)?;
        let mut parts = vec![organisation, name, "resolve", revision];
        parts.extend(path.split('/'));
        let url = self.url(&parts)?;
        self.get(url, usize::try_from(LONGEST_FILE).unwrap_or(usize::MAX))
            .await
    }
}
