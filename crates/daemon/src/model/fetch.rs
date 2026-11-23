//! The download runs in a child of the daemon in the mode [`child::FETCH_MODE`], with an empty
//! environment and no token, hub origin or proxy (Section 4, R-336).

pub mod child;
pub mod spawn;

use std::io::Write as _;
use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::follows;
use reqwest::{Client, Url, redirect};
use sha2::{Digest as _, Sha256};

fn client(hosts: &'static [&'static str]) -> Result<Client, Cause> {
    let policy = redirect::Policy::custom(move |attempt| {
        let url = attempt.url();
        if follows(
            attempt.previous().len(),
            url.scheme(),
            url.host_str(),
            hosts,
        ) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(policy)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| Cause::SourceFailed)
}

/// # Errors
/// `source_failed` when the source does not answer 200; `size` when it sends more than
/// `max_bytes`; `limit_disk` when the file cannot be written.
pub async fn download(
    address: &str,
    hosts: &'static [&'static str],
    max_bytes: u64,
    file: &mut std::fs::File,
) -> Result<String, Cause> {
    let url = Url::parse(address).map_err(|_| Cause::RuntimeNotConfigured)?;
    let mut response = client(hosts)?
        .get(url)
        .send()
        .await
        .map_err(|_| Cause::SourceFailed)?;
    if response.status().as_u16() != 200 {
        return Err(Cause::SourceFailed);
    }
    let mut written = Hashed::new(file, max_bytes);
    while let Some(chunk) = response.chunk().await.map_err(|_| Cause::SourceFailed)? {
        written.write(&chunk)?;
    }
    Ok(written.sha256())
}

pub struct Hashed<'a> {
    file: &'a mut std::fs::File,
    hash: Sha256,
    left: u64,
}

impl<'a> Hashed<'a> {
    #[must_use]
    pub fn new(file: &'a mut std::fs::File, max_bytes: u64) -> Self {
        Self {
            file,
            hash: Sha256::new(),
            left: max_bytes,
        }
    }

    /// # Errors
    /// `size` when the chunk passes the bound (nothing of it is written); `limit_disk` when
    /// the write fails.
    pub fn write(&mut self, chunk: &[u8]) -> Result<(), Cause> {
        self.left = self
            .left
            .checked_sub(chunk.len() as u64)
            .ok_or(Cause::Size)?;
        self.file.write_all(chunk).map_err(|_| Cause::LimitDisk)?;
        self.hash.update(chunk);
        Ok(())
    }

    #[must_use]
    pub fn sha256(self) -> String {
        hex(&self.hash.finalize())
    }
}

#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}
