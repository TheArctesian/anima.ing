use crate::{Error, Result, command};
pub(crate) use anima_os_maintenance::cutover::valid_system;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path, time::Duration};

pub const CHANNEL: &str = "https://releases.anima.ing/os/preview/x86_64-linux/";
pub const FEED: &str = "https://releases.anima.ing/os/preview/x86_64-linux/latest.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Release {
    pub format: u8,
    pub version: String,
    pub arch: String,
    pub channel: String,
    pub system: String,
    pub sha256: String,
    pub bytes: u64,
    pub unpacked_bytes: u64,
    pub url: String,
}

impl Release {
    fn validate(&self) -> Result<()> {
        if self.format != 1
            || self.arch != "x86_64-linux"
            || self.channel != "preview"
            || self.version.len() != 14
            || !self.version.bytes().all(|c| c.is_ascii_digit())
            || !valid_system(&self.system)
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || self.bytes == 0
            || self.bytes > 32 * 1024_u64.pow(3)
            || self.unpacked_bytes == 0
            || self.unpacked_bytes > 80 * 1024_u64.pow(3)
            || self.url != format!("{CHANNEL}{}/system.nar.xz", self.version)
        {
            return Err(Error::Refused(
                "The signed release is not compatible with this preview.",
            ));
        }
        Ok(())
    }
}

pub async fn verify(envelope: &[u8], key: &Path) -> Result<Release> {
    let signed: Envelope = serde_json::from_slice(envelope)?;
    let decode = |value: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(value)
            .map_err(|_| Error::Refused("The update signature envelope is invalid."))
    };
    let payload = decode(&signed.payload)?;
    let signature = decode(&signed.signature)?;
    let mut data = tempfile::NamedTempFile::new()?;
    let mut proof = tempfile::NamedTempFile::new()?;
    data.write_all(&payload)?;
    proof.write_all(&signature)?;
    command::run(
        "openssl",
        &[
            "dgst",
            "-sha256",
            "-verify",
            text(key)?,
            "-signature",
            text(proof.path())?,
            text(data.path())?,
        ],
    )
    .await
    .map_err(|_| Error::Refused("Update signature verification failed."))?;
    let release: Release = serde_json::from_slice(&payload)?;
    release.validate()?;
    Ok(release)
}

pub fn client() -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(3600))
        .user_agent("AnimaOS-Updater/2");
    if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
        let pem = crate::state::read_bounded(Path::new(&path), 1024 * 1024)?;
        builder = builder.add_root_certificate(reqwest::Certificate::from_pem(&pem)?);
    }
    Ok(builder.build()?)
}

pub async fn response(client: &reqwest::Client, url: &str) -> Result<reqwest::Response> {
    if !url.starts_with(CHANNEL) {
        return Err(Error::Refused(
            "The update address is outside the Anima OS preview channel.",
        ));
    }
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(Error::Refused(
            "The update server did not return the requested artifact.",
        ));
    }
    Ok(response)
}

pub async fn latest() -> Result<Release> {
    let request = async {
        let mut response = response(&client()?, FEED).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 65536 {
                return Err(Error::Refused("The update feed exceeds its size limit."));
            }
            bytes.extend_from_slice(&chunk);
        }
        let mut key = tempfile::NamedTempFile::new()?;
        key.write_all(include_bytes!(
            "../../../os/release/signing/preview-public.pem"
        ))?;
        verify(&bytes, key.path()).await
    };
    tokio::time::timeout(Duration::from_secs(30), request)
        .await
        .map_err(|_| Error::Refused("The update check timed out."))?
}

fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or(Error::Refused("The verification path is invalid."))
}
