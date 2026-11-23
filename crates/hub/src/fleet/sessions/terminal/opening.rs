//! The terminal's work before the WebSocket upgrade (K4 Sections 8.2 and 8.3, R-032, SEC-005, SEC-007, OD-9).
//! The credential is sent only after the presented key equals the accepted one, and is zeroed once the sign-in returns.
//! Nothing here logs; no error or message holds a host name, an address, the device's text or a secret (LOG-007).

use std::{sync::Arc, time::Duration};

use russh::{
    Channel,
    client::{self, Handle, Msg},
};
use tokio::time::timeout;

use super::{
    ending::{CREDENTIAL_MOVED, NO_CREDENTIAL, UNREADABLE_KEY},
    route::Terminal,
};
use crate::fleet::{
    credentials,
    devices::{self, Device},
    sessions::{self, PendingTicket, Protocol, end::Cause},
    ssh::{self, Observer, SshError, Target},
    trust,
};

/// K4 Section 8.2, spec Section 6.3.1; three unanswered end the connection.
const KEEPALIVE: Duration = Duration::from_secs(30);
/// Small, so a client that reads slowly stalls the device instead of growing a buffer (K4 Section 8.5).
const CHANNEL_BUFFER: usize = 4;

pub(super) struct SignedIn {
    pub(super) handle: Handle<Observer>,
    pub(super) channel: Channel<Msg>,
}

pub(super) enum Opening {
    SignedIn(SignedIn),
    /// Another host key, a certificate or an unreadable key: no credential was sent (SEC-007).
    HostKeyChanged,
    SignInRefused(&'static str),
}

/// Process-log messages hold no value, address, device text or token (LOG-007).
#[derive(Debug, thiserror::Error)]
pub(super) enum NotOpened {
    #[error("the device no longer exists")]
    DeviceGone,
    #[error("SSH is turned off for the device")]
    SshOff,
    #[error("no host key is accepted for the device")]
    NoHostKey,
    #[error("the device host does not resolve")]
    Unresolved,
    #[error("the device host resolves to a refused address")]
    RefusedTarget,
    #[error("the device's SSH port cannot be reached")]
    Unreachable,
    #[error("the device did not open the session in time")]
    Timeout,
    #[error("the device did not complete an SSH key exchange")]
    NoKeyExchange,
    #[error("the SSH connection failed during the sign-in or the channel open")]
    Lost,
    #[error("device store error")]
    Devices(#[source] devices::Error),
    #[error("session store error")]
    Sessions(#[source] sessions::Error),
    #[error("credential store error")]
    Credentials(#[source] credentials::Error),
}

impl NotOpened {
    pub(super) const fn message(&self) -> &'static str {
        match self {
            Self::DeviceGone => "The device was deleted before the SSH session opened.",
            Self::SshOff => "SSH was turned off for the device before the session opened.",
            Self::NoHostKey => {
                "The device has no accepted host key. Test the connection and accept its key."
            }
            Self::Unresolved => "The device's host name does not resolve.",
            Self::RefusedTarget => {
                "The device's host resolves to a loopback, link-local, multicast or unspecified \
                 address, or to one in a network the hub refuses (ANIMA_PROBE_DENY), which the \
                 hub must not reach."
            }
            Self::Unreachable => "The hub could not reach the device's SSH port.",
            Self::Timeout => "The device did not open the SSH session in time.",
            Self::NoKeyExchange => "The device did not complete an SSH key exchange.",
            Self::Lost => "The SSH connection to the device failed while the session opened.",
            Self::Devices(_) | Self::Sessions(_) | Self::Credentials(_) => {
                "The hub could not read the session's settings from its database."
            }
        }
    }
}

impl From<SshError> for NotOpened {
    fn from(error: SshError) -> Self {
        match error {
            SshError::Unresolved => Self::Unresolved,
            SshError::RefusedTarget => Self::RefusedTarget,
            SshError::Unreachable => Self::Unreachable,
            SshError::Timeout => Self::Timeout,
            SshError::Protocol => Self::NoKeyExchange,
            SshError::UnreadableKey | SshError::AuthFailed => Self::Lost,
        }
    }
}

/// Safe to drop at any await: dropping the connection's handle ends it (SEC-002).
pub(super) async fn open(
    terminal: &Terminal,
    ticket: &PendingTicket,
) -> Result<Opening, NotOpened> {
    timeout(terminal.timing.opening_of(ticket), opened(terminal, ticket))
        .await
        .map_err(|_| NotOpened::Timeout)?
}

async fn opened(terminal: &Terminal, ticket: &PendingTicket) -> Result<Opening, NotOpened> {
    let device = devices::get(&terminal.db, ticket.device)
        .await
        .map_err(|error| match error {
            devices::Error::NotFound(_) => NotOpened::DeviceGone,
            other => NotOpened::Devices(other),
        })?;
    if !device.ssh.enabled {
        return Err(NotOpened::SshOff);
    }
    let accepted = trust::host_key(&terminal.db, device.id)
        .await
        .map_err(|error| NotOpened::Sessions(error.into()))?
        .ok_or(NotOpened::NoHostKey)?
        .public_key;
    let target = Target {
        host: &device.host,
        port: device.ssh.port,
        policy: terminal.live.policy(),
    };
    let stream = if ticket.wake_sent {
        ssh::connect_waking(target, terminal.timing.reconnect).await?
    } else {
        ssh::connect(target).await?
    };
    let (observer, seen) = Observer::expecting(Some(accepted.clone()));
    let connected = client::connect_stream(config(), stream, observer).await;
    let presented = seen.await.map_err(|_| NotOpened::NoKeyExchange)?;
    if presented.as_ref() != Some(&accepted) {
        return Ok(Opening::HostKeyChanged);
    }
    let mut handle = connected.map_err(|_| NotOpened::NoKeyExchange)?;
    if let Some(refused) = sign_in(terminal, &device, &mut handle).await? {
        return Ok(Opening::SignInRefused(refused));
    }
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|_| NotOpened::Lost)?;
    Ok(Opening::SignedIn(SignedIn { handle, channel }))
}

/// The credential opens only for the host the connection went to (SEC-011).
async fn sign_in(
    terminal: &Terminal,
    device: &Device,
    handle: &mut Handle<Observer>,
) -> Result<Option<&'static str>, NotOpened> {
    let secret =
        match credentials::load_ssh(&terminal.db, &terminal.vault, (device.id, &device.host)).await
        {
            Err(credentials::Error::EnteredForOtherHost) => return Ok(Some(CREDENTIAL_MOVED)),
            loaded => loaded.map_err(NotOpened::Credentials)?,
        };
    let Some(secret) = secret else {
        return Ok(Some(NO_CREDENTIAL));
    };
    let signed_in = ssh::authenticate(handle, &device.ssh.username, &secret).await;
    drop(secret);
    match signed_in {
        Ok(()) => Ok(None),
        Err(SshError::AuthFailed) => Ok(Some(Cause::AuthFailed.message().of(Protocol::Ssh))),
        Err(SshError::UnreadableKey) => Ok(Some(UNREADABLE_KEY)),
        Err(_) => Err(NotOpened::Lost),
    }
}

/// No inactivity limit: an idle shell stays open while the keep-alive is answered.
pub(super) fn config() -> Arc<client::Config> {
    Arc::new(client::Config {
        preferred: ssh::preferred(),
        keepalive_interval: Some(KEEPALIVE),
        channel_buffer_size: CHANNEL_BUFFER,
        nodelay: true,
        ..client::Config::default()
    })
}
