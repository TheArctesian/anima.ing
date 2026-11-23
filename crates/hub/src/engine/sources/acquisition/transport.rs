//! The one way a request leaves the hub for a source (K-E1 Section 7).
//! No URL, header or body reaches a log or an error (LOG-007).
//! The client follows no redirect, takes no proxy from the environment, speaks HTTPS only and
//! marks the token header sensitive.

mod address;
mod at_account;

use std::{error::Error as StdError, net::IpAddr, sync::Arc, time::Duration};

use reqwest::{
    Client, Response, Url,
    dns::{Addrs, Name, Resolve, Resolving},
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use source_exchange::{Change, Declaration, Limit, Method, Request, Verb};
use zeroize::Zeroizing;

pub use self::address::address;
use super::{tokens::Tokens, verdict::Limits};
use crate::{
    engine::sources::live::Live,
    fleet::{
        credentials::Secret,
        targets::{AddressPolicy, resolve_checked},
    },
};

pub const BODY_CAP: usize = 8 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The most bytes of a change's JSON body: a 65,536-character text, escaped, fits.
pub const CHANGE_BODY_CAP: usize = 1024 * 1024;
/// The REST API version every request is written for (K-E1 Section 13.1).
const API_VERSION: &str = "2022-11-28";
const MEDIA_TYPE: &str = "application/vnd.github+json";

/// Why nothing usable came back. Fixed words: no URL, header or body (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Fault {
    #[error("the request names a path or a host the hub does not send to")]
    Refused,
    #[error("the source could not be reached")]
    Network,
}

#[derive(Debug, thiserror::Error)]
#[error("the source transport could not be built")]
pub struct BuildFailed(#[source] reqwest::Error);

pub struct Answer {
    pub status: u16,
    pub limits: Limits,
    pub content: Content,
    pub link: Option<String>,
}

impl Answer {
    /// Numbers only, never header text.
    #[must_use]
    pub fn limit(&self) -> Limit {
        Limit {
            remaining: self.limits.remaining,
            reset_at: self
                .limits
                .reset
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0)),
            retry_after_seconds: self.limits.retry_after,
        }
    }
}

pub enum Delivery {
    Answered(Answer),
    /// The request may have reached the source with no whole answer back; it is never sent again.
    Lost,
}

pub enum Content {
    /// Wiped on drop: a mint reply holds a token, a conversion reply a key.
    Body(Zeroizing<Vec<u8>>),
    TooLarge,
}

/// Clones share the tokens.
#[derive(Clone)]
pub struct Transport {
    client: Client,
    /// The host names of the declared API bases: the only destinations.
    allowed: Arc<[String]>,
    base_override: Option<Url>,
    /// The address policy of the hub, for a base that is of an account (Section 7b).
    policy: AddressPolicy,
    tokens: Tokens,
    live: Live,
}

/// Resolves only the allowed names, to addresses the policy permits (R-030, P3-003);
/// TLS name verification, not this, ties the connection to the declared host.
struct AllowedHosts {
    hosts: Arc<[String]>,
    policy: AddressPolicy,
}

#[derive(Debug, thiserror::Error)]
#[error("the host is not an allowed source")]
struct NotAllowed;

fn is_allowed(hosts: &[String], host: &str) -> bool {
    hosts
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(host))
}

impl Resolve for AllowedHosts {
    fn resolve(&self, name: Name) -> Resolving {
        let allowed = is_allowed(&self.hosts, name.as_str());
        let policy = self.policy.clone();
        Box::pin(async move {
            if !allowed {
                return Err(Box::new(NotAllowed) as Box<dyn StdError + Send + Sync>);
            }
            let addresses = resolve_checked(name.as_str(), 0, &policy)
                .await
                .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

fn client(
    hosts: Arc<[String]>,
    policy: AddressPolicy,
    https_only: bool,
) -> Result<Client, BuildFailed> {
    let version = HeaderValue::from_static(API_VERSION);
    let headers =
        HeaderMap::from_iter([(HeaderName::from_static("x-github-api-version"), version)]);
    Client::builder()
        .use_rustls_tls()
        .user_agent("anima-hub")
        .default_headers(headers)
        .no_proxy()
        .redirect(Policy::none())
        .https_only(https_only)
        .timeout(TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .dns_resolver(Arc::new(AllowedHosts { hosts, policy }))
        .build()
        .map_err(BuildFailed)
}

/// The host name of an `https` API base; none for an address, which would skip the resolver.
fn host_name(base: &str) -> Option<String> {
    let url = Url::parse(base)
        .ok()
        .filter(|url| url.scheme() == "https")?;
    let host = url.host_str()?;
    let literal = host.starts_with('[') || host.parse::<IpAddr>().is_ok();
    (!literal).then(|| host.to_ascii_lowercase())
}

impl Transport {
    /// The transport for the declared `api_bases`.
    ///
    /// # Errors
    /// [`BuildFailed`] when the TLS client cannot be built.
    pub fn new<'a>(
        api_bases: impl IntoIterator<Item = &'a str>,
        policy: AddressPolicy,
    ) -> Result<Self, BuildFailed> {
        let hosts: Arc<[String]> = api_bases.into_iter().filter_map(host_name).collect();
        Ok(Self {
            client: client(hosts.clone(), policy.clone(), true)?,
            allowed: hosts,
            base_override: None,
            policy,
            tokens: Tokens::default(),
            live: Live::default(),
        })
    }

    /// A transport to a loopback `stub` over plain HTTP, for tests.
    ///
    /// # Errors
    /// [`BuildFailed`] when the client cannot be built.
    #[cfg(any(test, feature = "test-support"))]
    pub fn to_stub(stub: Url, policy: AddressPolicy) -> Result<Self, BuildFailed> {
        let hosts: Arc<[String]> = stub.host_str().map(str::to_owned).into_iter().collect();
        Ok(Self {
            client: client(hosts.clone(), policy.clone(), false)?,
            allowed: hosts,
            base_override: Some(stub),
            policy,
            tokens: Tokens::default(),
            live: Live::default(),
        })
    }

    /// The installation tokens, in memory only (PRJ-018).
    #[must_use]
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    pub(in crate::engine::sources) const fn live(&self) -> &Live {
        &self.live
    }

    /// # Errors
    /// [`Fault::Refused`] when the base is not allowed or the path could leave it.
    pub fn url(&self, api_base: &str, request: &Request) -> Result<Url, Fault> {
        let base = match &self.base_override {
            Some(stub) => stub.clone(),
            None => Url::parse(api_base)
                .ok()
                .filter(|base| base.scheme() == "https")
                .ok_or(Fault::Refused)?,
        };
        let allowed = base
            .host_str()
            .is_some_and(|host| is_allowed(&self.allowed, host));
        if !allowed {
            return Err(Fault::Refused);
        }
        address(&base, &request.path, &request.query)
    }

    /// There is only `GET`: a provider cannot express a write.
    pub(in crate::engine::sources) async fn get(
        &self,
        request: &Request,
        url: Url,
        token: &Secret,
    ) -> Result<Answer, Fault> {
        let Method::Get = request.method;
        let sent = self
            .client
            .get(url)
            .header(AUTHORIZATION, bearer(token.expose())?)
            .header(ACCEPT, HeaderValue::from_static(MEDIA_TYPE));
        read(sent.send().await.map_err(|_| Fault::Network)?).await
    }

    pub(in crate::engine::sources) async fn mint(
        &self,
        url: Url,
        app_token: &str,
    ) -> Result<Answer, Fault> {
        let sent = self
            .client
            .post(url)
            .header(AUTHORIZATION, bearer(app_token.as_bytes())?)
            .header(ACCEPT, HeaderValue::from_static(MEDIA_TYPE));
        read(sent.send().await.map_err(|_| Fault::Network)?).await
    }

    /// No `Authorization` and no body: the one-time code in the path is the only proof (PRJ-039).
    pub(in crate::engine::sources) async fn convert(&self, url: Url) -> Result<Answer, Fault> {
        let sent = self
            .client
            .post(url)
            .header(ACCEPT, HeaderValue::from_static("application/json"));
        read(sent.send().await.map_err(|_| Fault::Network)?).await
    }
}

impl Transport {
    /// The transport renders the change itself: no caller hands it a path, a verb or a URL.
    /// [`Fault::Network`] when nothing was sent; [`Delivery::Lost`] once it may have been.
    pub(in crate::engine::sources) async fn change(
        &self,
        declaration: &Declaration,
        change: &Change,
        token: &Secret,
    ) -> Result<Delivery, Fault> {
        let changes = declaration.changes.ok_or(Fault::Refused)?;
        let rendered = (changes.render)(change);
        let target = Request::get(rendered.path(), &[]).map_err(|_| Fault::Refused)?;
        let url = self.url(declaration.api_base, &target)?;
        let body = match rendered.body() {
            Some(json) => Some(serde_json::to_vec(json).map_err(|_| Fault::Refused)?),
            None => None,
        };
        if body
            .as_ref()
            .is_some_and(|bytes| bytes.len() > CHANGE_BODY_CAP)
        {
            return Err(Fault::Refused);
        }
        let method = match rendered.verb() {
            Verb::Post => reqwest::Method::POST,
            Verb::Put => reqwest::Method::PUT,
            Verb::Patch => reqwest::Method::PATCH,
        };
        let mut sent = self
            .client
            .request(method, url)
            .header(AUTHORIZATION, bearer(token.expose())?)
            .header(ACCEPT, HeaderValue::from_static(MEDIA_TYPE));
        if let Some(bytes) = body {
            sent = sent
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(bytes);
        }
        match sent.send().await {
            Ok(response) => Ok(read(response)
                .await
                .map_or(Delivery::Lost, Delivery::Answered)),
            Err(error) if error.is_connect() => Err(Fault::Network),
            Err(_) => Ok(Delivery::Lost),
        }
    }
}

/// Marked sensitive, so it never appears in a `Debug` output.
fn bearer(token: &[u8]) -> Result<HeaderValue, Fault> {
    let mut text = Zeroizing::new(Vec::with_capacity(token.len() + 7));
    text.extend_from_slice(b"Bearer ");
    text.extend_from_slice(token);
    let mut value = HeaderValue::from_bytes(&text).map_err(|_| Fault::Refused)?;
    value.set_sensitive(true);
    Ok(value)
}

async fn read(mut response: Response) -> Result<Answer, Fault> {
    let status = response.status().as_u16();
    let limits = Limits::of(response.headers());
    let link = response
        .headers()
        .get(reqwest::header::LINK)
        .and_then(|header| header.to_str().ok())
        .map(str::to_owned);
    let content = content(&mut response).await?;
    Ok(Answer {
        status,
        limits,
        content,
        link,
    })
}

async fn content(response: &mut Response) -> Result<Content, Fault> {
    let expected = response
        .content_length()
        .and_then(|n| usize::try_from(n).ok());
    if expected.is_some_and(|n| n > BODY_CAP) {
        return Ok(Content::TooLarge);
    }
    let mut body = Zeroizing::new(Vec::with_capacity(expected.unwrap_or(0)));
    while let Some(chunk) = response.chunk().await.map_err(|_| Fault::Network)? {
        if body.len() + chunk.len() > BODY_CAP {
            return Ok(Content::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Content::Body(body))
}
