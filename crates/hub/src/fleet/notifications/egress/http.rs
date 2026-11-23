//! HTTP egress (CR-14): one request over a client pinned to the checked addresses of one host,
//! always over TLS. The client resolves no name, follows no redirect and uses no proxy.

use std::{error::Error, future, net::SocketAddr, sync::Arc};

use reqwest::{
    Certificate, Client, Method, Response, Url,
    dns::{Name, Resolve, Resolving},
    header::HeaderMap,
    redirect::Policy,
};
use tokio::time::{Instant, timeout_at};

use super::{Egress, SendError, TIME_LIMIT, causes, tls_broke};
use crate::fleet::targets::{TargetError, resolve_checked};

/// The most reply body bytes read; the rest is never read (D-17).
pub(super) const REPLY_CAP: usize = 4 * 1024;

/// The client looks up nothing, so a host is reached only at the addresses pinned for it.
#[derive(Debug, thiserror::Error)]
#[error("the notification client resolves no names")]
struct LookupRefused;

/// No `Debug`: the URL and the header values may be secrets (LOG-007).
pub(crate) struct HttpRequest {
    pub(crate) method: Method,
    pub(crate) url: Url,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
}

struct NoLookup;

impl Resolve for NoLookup {
    fn resolve(&self, _name: Name) -> Resolving {
        Box::pin(future::ready(Err(
            Box::new(LookupRefused) as Box<dyn Error + Send + Sync>
        )))
    }
}

impl Egress {
    /// One deadline of [`TIME_LIMIT`] covers the lookup, the connection, the head and the body
    /// read (N-8). A 2xx status decides success even when the body then stalls, so an answered
    /// send never turns into a retry and a duplicate (S-B).
    ///
    /// # Errors
    /// [`SendError::Target`] with no connection made for a refused or unresolved host; else the
    /// [`SendError`] of the failed step.
    pub(crate) async fn send_http(&self, request: HttpRequest) -> Result<(), SendError> {
        let deadline = Instant::now() + TIME_LIMIT;
        let response = timeout_at(deadline, self.request(request))
            .await
            .map_err(|_| SendError::TimedOut)??;
        judge(response, deadline).await
    }

    async fn request(&self, request: HttpRequest) -> Result<Response, SendError> {
        let HttpRequest {
            method,
            url,
            mut headers,
            body,
        } = request;
        // Always TLS (CR-14 item 6): a plain `http` URL gets no lookup and no connection.
        if url.scheme() != "https" {
            return Err(SendError::Url);
        }
        let host = url.host_str().ok_or(SendError::Url)?;
        let port = url.port_or_known_default().ok_or(SendError::Url)?;
        let addresses = resolve_checked(host, port, &self.policy).await?;
        let client = self.pinned_client(url.domain(), &addresses)?;
        for value in headers.values_mut() {
            value.set_sensitive(true);
        }
        client
            .request(method, url)
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|error| classify(&error))
    }

    /// An IP-literal host (`domain` is `None`) is reached at the address it names.
    pub(super) fn pinned_client(
        &self,
        domain: Option<&str>,
        addresses: &[SocketAddr],
    ) -> Result<Client, SendError> {
        let mut builder = Client::builder()
            .use_rustls_tls()
            .tls_built_in_root_certs(true)
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
            let root = Certificate::from_der(der).map_err(|_| SendError::Setup)?;
            builder = builder.add_root_certificate(root);
        }
        builder.build().map_err(|_| SendError::Setup)
    }
}

async fn judge(mut response: Response, deadline: Instant) -> Result<(), SendError> {
    let status = response.status();
    if !status.is_success() {
        return Err(SendError::HttpStatus(status.as_u16()));
    }
    let _ = timeout_at(deadline, discard_reply(&mut response)).await;
    Ok(())
}

pub(super) async fn discard_reply(response: &mut Response) -> usize {
    let mut read = 0;
    while read < REPLY_CAP {
        match response.chunk().await {
            Ok(Some(chunk)) => read += chunk.len(),
            Ok(None) | Err(_) => break,
        }
    }
    read
}

/// Judged from the error's causes, never its text, which holds the URL.
pub(super) fn classify(error: &reqwest::Error) -> SendError {
    if causes(error).any(<dyn Error>::is::<LookupRefused>) {
        SendError::Target(TargetError::Unresolved)
    } else if tls_broke(error) {
        SendError::Tls
    } else if error.is_connect() {
        SendError::Connect
    } else {
        SendError::Exchange
    }
}
