//! In-memory credential relay to the isolated checkout's root supervisor.
use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::source_jobs::{
    Attempt, CHECKOUT_READY, MAX_CHECKOUT_TOKEN_BYTES, private_checkout, valid_checkout_token,
};
use serde_json::Value;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    time::timeout,
};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{Error, Phase, Result, authorized, journal, snapshot};
use crate::linux::workspace::checkout::Credential;

const TRANSFER: Duration = Duration::from_secs(5);
const WAIT: Duration = Duration::from_secs(50);

pub(super) struct Listener {
    listener: UnixListener,
    id: Uuid,
}

impl Listener {
    pub(super) fn bind(id: Uuid) -> Result<Self> {
        Ok(Self {
            listener: UnixListener::bind(path(id))?,
            id,
        })
    }

    pub(super) async fn receive(self) -> Result<(Option<Credential>, UnixStream)> {
        let (mut stream, _) = timeout(WAIT, self.listener.accept())
            .await
            .map_err(|_| Error::Refused("No private checkout credential was delivered."))??;
        drop(self);
        root(&stream)?;
        let credential = timeout(TRANSFER, receive(&mut stream))
            .await
            .map_err(|_| Error::Refused("The private checkout credential did not arrive."))??;
        Ok((credential, stream))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = remove(self.id);
    }
}

fn path(id: Uuid) -> PathBuf {
    PathBuf::from(format!("/run/anima-os/checkout-{id}.sock"))
}

pub(super) fn remove(id: Uuid) -> Result<()> {
    match std::fs::remove_file(path(id)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn receive(stream: &mut UnixStream) -> Result<Option<Credential>> {
    let expires = stream.read_i64().await?;
    let value = read_secret(stream).await?;
    if expires == 0 && value.is_empty() {
        return Ok(None);
    }
    let expires = DateTime::<Utc>::from_timestamp_millis(expires)
        .ok_or(Error::Refused("Invalid source checkout expiry."))?;
    Credential::new(value, expires).map(Some)
}

pub(super) async fn acknowledge(stream: &mut UnixStream) {
    // The daemon can lose its reply after delivery. The supervisor already owns the next step.
    let _ = timeout(TRANSFER, stream.write_u8(1)).await;
}

pub(super) async fn relay(
    (device, origin): (Uuid, &str),
    (identity, expires): (Attempt, Option<DateTime<Utc>>),
    io: (
        &mut (impl AsyncRead + Unpin),
        &mut (impl AsyncWrite + Unpin),
    ),
) -> Result<Value> {
    let record = timeout(
        TRANSFER,
        journal(|| authorized(identity.execution, device, origin)),
    )
    .await
    .map_err(|_| Error::Refused("The source-job journal is busy."))??;
    let now = Utc::now();
    // Work reports carry whole-second leases. A secret reply can retain subsecond precision.
    let expires = expires.map(|until| until.min(record.lease.grant.until));
    if record.job.id != identity.job
        || record.job.attempt != identity.attempt
        || record.phase != Phase::AwaitingCheckoutCredential
        || record.lease.expired()
        || !private_checkout(&record.job)
        || expires.is_some_and(|until| until <= now || until > now + TimeDelta::seconds(90))
    {
        return Err(Error::Refused(
            "This source job is not waiting for a checkout credential.",
        ));
    }
    let mut stream = timeout(TRANSFER, UnixStream::connect(path(identity.execution)))
        .await
        .map_err(|_| Error::Refused("The checkout supervisor is unavailable."))??;
    root(&stream)?;
    timeout(TRANSFER, send(&mut stream, expires, io))
        .await
        .map_err(|_| Error::Refused("The checkout credential handoff did not complete."))??;
    let state = timeout(TRANSFER, journal(|| snapshot(identity.execution)))
        .await
        .map_err(|_| Error::Refused("The source-job journal is busy."))??;
    Ok(serde_json::to_value(state)?)
}

async fn send(
    stream: &mut UnixStream,
    expires: Option<DateTime<Utc>>,
    (reading, writing): (
        &mut (impl AsyncRead + Unpin),
        &mut (impl AsyncWrite + Unpin),
    ),
) -> Result<()> {
    if let Some(expires) = expires {
        writing.write_all(CHECKOUT_READY).await?;
        let value = read_secret(reading).await?;
        if !valid_checkout_token(&value) || expires <= Utc::now() {
            return Err(Error::Refused("Invalid source checkout credential."));
        }
        stream.write_i64(expires.timestamp_millis()).await?;
        stream.write_all(&value).await?;
    } else {
        stream.write_i64(0).await?;
    }
    stream.shutdown().await?;
    if stream.read_u8().await? != 1 {
        return Err(Error::Refused(
            "The checkout supervisor did not accept the handoff.",
        ));
    }
    Ok(())
}

async fn read_secret(reader: &mut (impl AsyncRead + Unpin)) -> Result<Zeroizing<Vec<u8>>> {
    let bound = MAX_CHECKOUT_TOKEN_BYTES + 1;
    let mut bytes = Zeroizing::new(Vec::with_capacity(bound));
    reader.take(bound as u64).read_to_end(&mut bytes).await?;
    Ok(bytes)
}

fn root(stream: &UnixStream) -> Result<()> {
    if stream.peer_cred()?.uid() != 0 {
        return Err(Error::Refused("Source checkout requires the root owner."));
    }
    Ok(())
}
