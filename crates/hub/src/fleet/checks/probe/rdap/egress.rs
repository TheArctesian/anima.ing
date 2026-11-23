//! RDAP egress (P3-003, P-6): each hop, redirects included, is a new target pinned to its checked addresses,
//! so neither a redirect nor a second DNS answer can reach an address the policy refuses.
//! A body over the cap is refused, never cut. A [`FetchError`] names no URL, host, address or reply (LOG-007).

use std::{error::Error, future, io, net::SocketAddr, sync::Arc};

use reqwest::{
    Certificate, Client, Response, StatusCode, Url,
    dns::{Name, Resolve, Resolving},
    header::{ACCEPT, LOCATION},
    redirect::Policy,
};

use crate::fleet::targets::{AddressPolicy, TargetError, resolve_checked};

/// The most redirects one fetch follows (D-29).
pub(super) const REDIRECTS_MAX: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum FetchError {
    #[error("the RDAP address is not permitted")]
    Target(#[from] TargetError),
    #[error("the RDAP address is not an HTTPS URL with a host")]
    Address,
    #[error("the RDAP server redirected more than {REDIRECTS_MAX} times")]
    Redirects,
    #[error("the RDAP server answered with HTTP status {0}")]
    Status(u16),
    #[error("the RDAP answer is longer than the hub reads")]
    TooLong,
    #[error("TLS failed: the certificate is not trusted for the host name, or the handshake broke")]
    Tls,
    #[error("the RDAP connection could not be prepared")]
    Setup,
    #[error("the connection failed or broke")]
    Connect,
    #[error("the RDAP exchange failed")]
    Exchange,
}

#[derive(Clone, Debug)]
pub(super) struct Egress {
    policy: AddressPolicy,
    extra_roots: Vec<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
#[error("the RDAP client resolves no names")]
struct LookupRefused;

/// The client looks up nothing, so a host is reached only at its pinned addresses.
struct NoLookup;

impl Resolve for NoLookup {
    fn resolve(&self, _name: Name) -> Resolving {
        Box::pin(future::ready(Err(
            Box::new(LookupRefused) as Box<dyn Error + Send + Sync>
        )))
    }
}

impl Egress {
    pub(super) const fn new(policy: AddressPolicy) -> Self {
        Self {
            policy,
            extra_roots: Vec::new(),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub(super) fn trusting(mut self, der: Vec<u8>) -> Self {
        self.extra_roots.push(der);
        self
    }

    /// The caller bounds the time.
    pub(super) async fn get(&self, url: &Url, cap: usize) -> Result<Vec<u8>, FetchError> {
        let mut hop = url.clone();
        for _ in 0..=REDIRECTS_MAX {
            let response = self.request(&hop).await?;
            match next_hop(&response, &hop)? {
                Some(next) => hop = next,
                None => return read_capped(response, cap).await,
            }
        }
        Err(FetchError::Redirects)
    }

    async fn request(&self, url: &Url) -> Result<Response, FetchError> {
        let plain =
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none();
        let host = url
            .host_str()
            .filter(|_| plain)
            .ok_or(FetchError::Address)?;
        let port = url.port_or_known_default().ok_or(FetchError::Address)?;
        let addresses = resolve_checked(host, port, &self.policy).await?;
        self.pinned_client(url.domain(), &addresses)?
            .get(url.clone())
            .header(ACCEPT, "application/rdap+json, application/json")
            .send()
            .await
            .map_err(|error| classify(&error))
    }

    /// An IP-literal host (`domain` is `None`) is reached at the address it names, already checked.
    pub(super) fn pinned_client(
        &self,
        domain: Option<&str>,
        addresses: &[SocketAddr],
    ) -> Result<Client, FetchError> {
        let mut builder = Client::builder()
            .use_rustls_tls()
            .tls_built_in_root_certs(true)
            .https_only(true)
            .user_agent("anima-hub")
            .no_proxy()
            .redirect(Policy::none())
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .dns_resolver(Arc::new(NoLookup));
        if let Some(domain) = domain {
            builder = builder.resolve_to_addrs(domain, addresses);
        }
        for der in &self.extra_roots {
            let root = Certificate::from_der(der).map_err(|_| FetchError::Setup)?;
            builder = builder.add_root_certificate(root);
        }
        builder.build().map_err(|_| FetchError::Setup)
    }
}

fn next_hop(response: &Response, current: &Url) -> Result<Option<Url>, FetchError> {
    let status = response.status();
    if status.is_success() {
        return Ok(None);
    }
    let redirect = matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    );
    let refused = FetchError::Status(status.as_u16());
    if !redirect {
        return Err(refused);
    }
    let location = response.headers().get(LOCATION).ok_or(refused)?;
    let location = location.to_str().map_err(|_| refused)?;
    current.join(location).map(Some).map_err(|_| refused)
}

async fn read_capped(mut response: Response, cap: usize) -> Result<Vec<u8>, FetchError> {
    let declared = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok());
    if declared.is_some_and(|length| length > cap) {
        return Err(FetchError::TooLong);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| classify(&error))? {
        if chunk.len() > cap - body.len() {
            return Err(FetchError::TooLong);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Judged from the error's causes, never its text, which holds the URL.
pub(super) fn classify(error: &reqwest::Error) -> FetchError {
    if causes(error).any(<dyn Error>::is::<LookupRefused>) {
        FetchError::Target(TargetError::Unresolved)
    } else if causes(error).any(<dyn Error>::is::<rustls::Error>) {
        FetchError::Tls
    } else if error.is_connect() {
        FetchError::Connect
    } else {
        FetchError::Exchange
    }
}

/// An I/O error leads to the error it carries, which its own `source` skips.
fn causes<'a>(error: &'a (dyn Error + 'static)) -> impl Iterator<Item = &'a (dyn Error + 'static)> {
    std::iter::successors(Some(error), |&cause| {
        match cause.downcast_ref::<io::Error>() {
            Some(io) => io.get_ref().map(|inner| inner as &(dyn Error + 'static)),
            None => cause.source(),
        }
    })
}
