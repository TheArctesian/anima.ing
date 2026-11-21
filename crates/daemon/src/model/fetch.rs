//! The fetch process of model work (Section 4, R-NEW-fetchprocess, task L1-6b). The download
//! runs in a child of the daemon, made from the same program in the mode
//! [`child::FETCH_MODE`], with an empty environment and no token, hub origin or proxy. The rules
//! are pure, in `daemon_core::model_work::releases::fetch`.
//!
//! [`child`] is the child: it downloads, verifies and unpacks. [`spawn`] is the daemon side: it
//! starts the child, gives it its order and ends it at the deadline. [`download`] is the HTTPS
//! client of the child: no proxy, the roots of the program, at most 5 redirects and only to a
//! host of the fixed list, never to an IP address; at most `sizeBytes` read.

pub mod child;
pub mod spawn;

use std::io::Write as _;
use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::{MAX_REDIRECTS, redirect_permitted};
use reqwest::{Client, Url, redirect};
use sha2::{Digest as _, Sha256};

fn client(hosts: &'static [&'static str]) -> Result<Client, Cause> {
    let policy = redirect::Policy::custom(move |attempt| {
        let url = attempt.url();
        if attempt.previous().len() >= MAX_REDIRECTS
            || !redirect_permitted(url.scheme(), url.host_str(), hosts)
        {
            attempt.stop()
        } else {
            attempt.follow()
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

/// Downloads `address` into `file` and gives the SHA-256 in lower-case hexadecimal.
///
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
    let mut hash = Sha256::new();
    let mut read = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|_| Cause::SourceFailed)? {
        read += chunk.len() as u64;
        if read > max_bytes {
            return Err(Cause::Size);
        }
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|_| Cause::LimitDisk)?;
    }
    Ok(hex(&hash.finalize()))
}

/// Lower-case hexadecimal of `bytes`.
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
