//! Keep registry requests and credentials at the administrator-configured origin.
use std::{fs::OpenOptions, io::Read as _, os::unix::fs::OpenOptionsExt as _, path::Path};

use reqwest::{Url, header::HeaderValue};
use zeroize::Zeroizing;

use super::Error;

pub(super) struct Destination {
    pub(super) origin: Url,
    pub(super) repository: String,
}

impl Destination {
    pub(super) fn parse(origin: &str, repository: &str) -> Result<Self, Error> {
        let url = Url::parse(origin).map_err(|_| Error::RegistryConfig)?;
        let host = url.host_str().ok_or(Error::RegistryConfig)?;
        if url.scheme() != "https"
            || url.path() != "/"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (!host.contains(['.', ':']) && host != "localhost" && url.port().is_none())
            || !valid_repository(repository)
        {
            return Err(Error::RegistryConfig);
        }
        let destination = Self {
            origin: url,
            repository: repository.to_owned(),
        };
        let sample = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        if !compose_rules::forms::is_image(&destination.reference(sample)) {
            return Err(Error::RegistryConfig);
        }
        Ok(destination)
    }

    pub(super) fn object(&self, path: &str) -> Result<Url, Error> {
        self.origin
            .join(&format!("/v2/{}/{path}", self.repository))
            .map_err(|_| Error::Registry)
    }

    pub(super) fn upload(&self, location: &str) -> Result<Url, Error> {
        let prefix = format!("/v2/{}/blobs/uploads/", self.repository);
        let url = self
            .object("blobs/uploads/")?
            .join(location)
            .map_err(|_| Error::Registry)?;
        let session = url.path().strip_prefix(&prefix).ok_or(Error::Registry)?;
        if url.origin() != self.origin.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || session.is_empty()
            || !session
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || url.query_pairs().any(|(key, _)| key == "digest")
        {
            return Err(Error::Registry);
        }
        Ok(url)
    }

    pub(super) fn reference(&self, digest: &str) -> String {
        let url = self.origin.as_str();
        let authority = &url[self.origin.scheme().len() + 3..url.len() - 1];
        format!("{authority}/{}@{digest}", self.repository)
    }
}

fn valid_repository(repository: &str) -> bool {
    repository.len() <= 255
        && repository.split('/').all(|part| {
            let letter = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
            let bytes = part.as_bytes();
            bytes.first().copied().is_some_and(letter)
                && bytes.last().copied().is_some_and(letter)
                && bytes
                    .iter()
                    .all(|&byte| letter(byte) || matches!(byte, b'.' | b'_' | b'-'))
        })
}

pub(super) fn authorization(path: &Path) -> Result<HeaderValue, Error> {
    const LIMIT: usize = 8192;
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits().cast_signed())
        .open(path)
        .map_err(|_| Error::RegistryConfig)?;
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err(Error::RegistryConfig);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::RegistryConfig)?;
    if bytes.len() > LIMIT {
        return Err(Error::RegistryConfig);
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::RegistryConfig)?
        .trim_end_matches(['\r', '\n']);
    let (scheme, token) = text.split_once(' ').ok_or(Error::RegistryConfig)?;
    if !matches!(scheme, "Basic" | "Bearer")
        || token.is_empty()
        || !token.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(Error::RegistryConfig);
    }
    let mut value = HeaderValue::from_str(text).map_err(|_| Error::RegistryConfig)?;
    value.set_sensitive(true);
    Ok(value)
}
