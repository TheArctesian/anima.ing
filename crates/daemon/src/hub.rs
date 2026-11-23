//! The daemon's one HTTP client and its calls to the hub (K8 Section 1; K-E5 Section 2.1).
//!
//! In the link mode the client uses only the Unix socket of `anima-link` (R-172), and writes only
//! to a peer that passed its check ([`peer`]). It logs nothing; callers log at their own boundary,
//! never a code, a token or a body (LOG-007). Web PKI roots, no redirect, no proxy, no cookie and
//! [`TIMEOUT`] are MET-020.

pub mod answer;
pub mod body;
pub mod enrolment;
pub mod link_socket;
#[cfg(unix)]
pub mod peer;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
pub mod work;

use std::path::Path;
use std::time::Duration;

use bytes::Bytes;
use daemon_core::wire::{
    Batch, DaemonToken, EnrolRequest, Enrolled, EnrolmentCode, FetchedKey, KeyFetch, PushAccepted,
    RenewRequest, Renewed,
};
use reqwest::{Client, Url, redirect};
use zeroize::Zeroizing;

use crate::config::origin::Reach;
use answer::{Answer, Cause, answer};
use body::Body;
use link_socket::LinkSocketError;
#[cfg(unix)]
use peer::{Expected, PeerError};
#[cfg(unix)]
use unix::Local;
#[cfg(windows)]
use windows::Local;

/// One request, connection and answer together (K8 Section 7).
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// No request goes to this address: the client connects to the socket and sets the `Host` itself.
const ON_LINK_SOCKET: &str = "http://127.0.0.1/";

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("the HTTP client could not be built")]
    Build,
    #[error(transparent)]
    LinkSocket(#[from] LinkSocketError),
    #[cfg(unix)]
    #[error(transparent)]
    Peer(#[from] PeerError),
}

struct Call<'a> {
    route: &'static str,
    bearer: Option<&'a DaemonToken>,
    body: Zeroizing<Vec<u8>>,
    ok: &'static [u16],
}

pub struct HubClient {
    http: Client,
    base: Url,
    local: Option<Local>,
}

impl HubClient {
    /// Pass an [`Origin`](crate::config::origin::Origin)'s URL; its parser owns what a hub
    /// origin is.
    ///
    /// # Errors
    /// [`HubError`] when the TLS roots cannot be set up.
    pub fn new(base: Url, limit: Duration) -> Result<Self, HubError> {
        let http = Client::builder()
            .redirect(redirect::Policy::none())
            .no_proxy()
            .timeout(limit)
            .build()
            .map_err(|_| HubError::Build)?;
        Ok(Self {
            http,
            base,
            local: None,
        })
    }

    /// Each request connects only to `socket`, and on Unix only to the peer this daemon expects.
    ///
    /// # Errors
    /// [`HubError::LinkSocket`] when a different user could listen at the path;
    /// [`HubError::Peer`] when the user or the signature of this daemon cannot be read.
    pub fn over_link(socket: &Path, limit: Duration) -> Result<Self, HubError> {
        #[cfg(unix)]
        return Self::over_link_to(socket, limit, Expected::of_this_daemon()?);
        #[cfg(windows)]
        {
            link_socket::trusted(socket)?;
            let mut client = Self::on_link_socket(limit)?;
            client.local = Some(Local::new(socket.to_path_buf(), limit));
            Ok(client)
        }
    }

    /// # Errors
    /// [`HubError::LinkSocket`] when a different user could listen at the path.
    #[cfg(unix)]
    pub fn over_link_to(
        socket: &Path,
        limit: Duration,
        expected: Expected,
    ) -> Result<Self, HubError> {
        link_socket::trusted(socket)?;
        let mut client = Self::on_link_socket(limit)?;
        client.local = Some(Local::new(socket.to_path_buf(), limit, expected));
        Ok(client)
    }

    fn on_link_socket(limit: Duration) -> Result<Self, HubError> {
        Self::new(
            Url::parse(ON_LINK_SOCKET).map_err(|_| HubError::Build)?,
            limit,
        )
    }

    /// `own_socket` serves a daemon that was enrolled with an old link origin.
    ///
    /// # Errors
    /// [`HubError`] as [`HubClient::new`] and [`HubClient::over_link`].
    pub fn reaching(
        reach: Reach<'_>,
        own_socket: &Path,
        limit: Duration,
    ) -> Result<Self, HubError> {
        match reach {
            Reach::Https(url) => Self::new(url.clone(), limit),
            Reach::LinkSocket(socket) => Self::over_link(socket, limit),
            Reach::OwnLinkSocket => Self::over_link(own_socket, limit),
        }
    }

    /// The code is spent by this call (K8 Section 3).
    pub async fn enrol(&self, code: EnrolmentCode) -> Answer<Enrolled> {
        let call = Call {
            route: "daemon/v1/enrol",
            bearer: None,
            body: json(&EnrolRequest { code }),
            ok: &[200],
        };
        self.post(call, |_, body| read_json(body)).await
    }

    pub async fn push(&self, token: &DaemonToken, batch: &Batch) -> Answer<Option<PushAccepted>> {
        let call = Call {
            route: "daemon/v1/push",
            bearer: Some(token),
            body: json(batch),
            ok: &[204, 200],
        };
        // An `Accepted` that does not read is ignored; the batch is stored either way.
        self.post(call, |status, body| {
            Some((status == 200).then(|| read_json(body)).flatten())
        })
        .await
    }

    pub async fn fetch_key(&self, token: &DaemonToken, asked: &KeyFetch) -> Answer<FetchedKey> {
        let call = Call {
            route: "daemon/v1/keys/fetch",
            bearer: Some(token),
            body: json(asked),
            ok: &[200],
        };
        self.post(call, |_, body| read_json(body)).await
    }

    pub async fn renew(&self, token: &DaemonToken) -> Answer<Renewed> {
        let call = Call {
            route: "daemon/v1/renew",
            bearer: Some(token),
            body: json(&RenewRequest {}),
            ok: &[200],
        };
        self.post(call, |_, body| read_json(body)).await
    }

    async fn post<T>(
        &self,
        call: Call<'_>,
        read_ok: impl FnOnce(u16, &[u8]) -> Option<T>,
    ) -> Answer<T> {
        if let Some(local) = &self.local {
            return local.post(call, read_ok).await;
        }
        let Ok(url) = self.base.join(call.route) else {
            return Answer::Failed(Cause::Unreadable);
        };
        let mut request = self
            .http
            .post(url)
            .header("content-type", "application/json")
            // The daemon's one copy is wiped on drop; the buffers of hyper and rustls are not.
            .body(Bytes::from_owner(call.body));
        if let Some(token) = call.bearer {
            request = request.bearer_auth(token.expose());
        }
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(error) => return Answer::Failed(cause(&error)),
        };
        let status = response.status().as_u16();
        let mut bytes = Body::new();
        while !bytes.is_full() {
            match response.chunk().await {
                Ok(Some(chunk)) => bytes.add(&chunk),
                Ok(None) => break,
                Err(error) => return Answer::Failed(cause(&error)),
            }
        }
        if call.ok.contains(&status) {
            let read = read_ok(status, bytes.bytes());
            return read.map_or(Answer::Failed(Cause::Unreadable), Answer::Accepted);
        }
        answer(status, bytes.bytes(), call.ok[0], |_| None)
    }
}

/// Whether the peer at `socket` is one that `expected` admits; nothing is written to it.
///
/// # Errors
/// [`HubError::LinkSocket`] when a different user could listen at the path.
#[cfg(unix)]
pub async fn link_admits(socket: &Path, expected: &Expected) -> Result<bool, HubError> {
    link_socket::trusted(socket)?;
    let checking = unix::checked(socket, expected);
    Ok(matches!(
        tokio::time::timeout(TIMEOUT, checking).await,
        Ok(Ok(_))
    ))
}

fn cause(error: &reqwest::Error) -> Cause {
    if error.is_timeout() {
        Cause::Timeout
    } else {
        Cause::Network
    }
}

fn json(value: &impl serde::Serialize) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(serde_json::to_vec(value).expect("a K8 request body serialises"))
}

fn read_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    serde_json::from_slice(bytes).ok()
}
