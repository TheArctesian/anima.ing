//! Opening a link (P2-003, P2-004, OD-9): the host is resolved once, each address checked, and
//! the accepted key pinned in the key exchange (SEC-007). Nothing here logs.

use std::{sync::Arc, time::Duration};

use russh::client::{self, Handle};
use sqlx::PgPool;
use tokio::time::timeout;

use super::Error;
use crate::fleet::{
    credentials,
    devices::Device,
    ssh::{self, Observer, SshError},
    targets::{self, AddressPolicy, Reached},
    trust,
};

const OPENING: Duration = Duration::from_secs(10);
/// So an address that drops packets does not use up [`OPENING`] before the next is tried.
const ADDRESS_WAIT: Duration = Duration::from_secs(3);
const KEEPALIVE: Duration = Duration::from_secs(10);
const KEEPALIVE_MAX: usize = 3;
/// So a caller that keeps a link too long does not hold the device's connection open.
const IDLE: Duration = Duration::from_secs(60);
/// So a device that floods a channel stalls instead of growing a buffer.
const CHANNEL_BUFFER: usize = 8;

pub(super) async fn open(
    db: &PgPool,
    vault: &credentials::Vault,
    policy: &AddressPolicy,
    device: &Device,
) -> Result<Handle<Observer>, Error> {
    timeout(OPENING, opened(db, vault, policy, device))
        .await
        .map_err(|_| Error::Timeout)?
}

async fn opened(
    db: &PgPool,
    vault: &credentials::Vault,
    policy: &AddressPolicy,
    device: &Device,
) -> Result<Handle<Observer>, Error> {
    if !device.ssh.enabled {
        return Err(Error::SshOff);
    }
    let accepted = trust::host_key(db, device.id)
        .await
        .map_err(Error::Trust)?
        .ok_or(Error::NoHostKey)?
        .public_key;
    let stream = reach(policy, device).await?;
    let (observer, seen) = Observer::expecting(Some(accepted.clone()));
    let connected = client::connect_stream(config(), stream, observer).await;
    let presented = seen.await.map_err(|_| Error::NoKeyExchange)?;
    if presented.as_ref() != Some(&accepted) {
        return Err(Error::HostKeyChanged);
    }
    let mut handle = connected.map_err(|_| Error::NoKeyExchange)?;
    sign_in(db, vault, device, &mut handle).await?;
    Ok(handle)
}

async fn reach(policy: &AddressPolicy, device: &Device) -> Result<Reached, Error> {
    let addresses = targets::resolve_checked(&device.host, device.ssh.port, policy)
        .await
        .map_err(SshError::from)?;
    for address in addresses {
        if let Ok(Ok(stream)) = timeout(ADDRESS_WAIT, targets::connect(policy, address)).await {
            return Ok(stream);
        }
    }
    Err(Error::Unreachable)
}

/// The credential opens only for the host this link resolved (SEC-011), and is zeroed before this returns.
async fn sign_in(
    db: &PgPool,
    vault: &credentials::Vault,
    device: &Device,
    handle: &mut Handle<Observer>,
) -> Result<(), Error> {
    let secret = credentials::load_ssh(db, vault, (device.id, &device.host))
        .await
        .map_err(|error| match error {
            credentials::Error::EnteredForOtherHost => Error::CredentialForOtherHost,
            other => Error::Credentials(other),
        })?
        .ok_or(Error::NoCredential)?;
    let signed_in = ssh::authenticate(handle, &device.ssh.username, &secret).await;
    drop(secret);
    signed_in.map_err(Error::from)
}

fn config() -> Arc<client::Config> {
    Arc::new(client::Config {
        preferred: ssh::preferred(),
        keepalive_interval: Some(KEEPALIVE),
        keepalive_max: KEEPALIVE_MAX,
        inactivity_timeout: Some(IDLE),
        channel_buffer_size: CHANNEL_BUFFER,
        ..client::Config::default()
    })
}
