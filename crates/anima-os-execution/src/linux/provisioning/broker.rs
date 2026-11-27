//! Narrow lifecycle socket; filesystem quota privileges are absent from the runner broker.
mod handoff;

use super::{Error, Result, workspace::checkout::ManagedCredential};
use agent_threads::managed::workspace::{Action, CheckoutGrant, Command, LocalRequest, SOCKET};
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::source_jobs::{CHECKOUT_READY, MAX_CHECKOUT_TOKEN_BYTES};
use serde_json::json;
use std::{os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Semaphore,
};
use zeroize::Zeroizing;

pub(crate) async fn run() -> Result<()> {
    let user = nix::unistd::User::from_name("anima-daemon")
        .map_err(std::io::Error::from)?
        .ok_or(Error::Refused(
            "The enrolled daemon account is unavailable.",
        ))?;
    let uid = user.uid.as_raw();
    tokio::try_join!(lifecycle(uid), handoff::run(uid))?;
    Ok(())
}

fn listen(path: &str) -> Result<UnixListener> {
    match std::fs::remove_file(path) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
    Ok(listener)
}

fn authenticated(stream: &UnixStream, uid: u32) -> bool {
    stream.peer_cred().is_ok_and(|peer| peer.uid() == uid)
}

async fn lifecycle(uid: u32) -> Result<()> {
    let listener = listen(SOCKET)?;
    let slots = Arc::new(Semaphore::new(4));
    loop {
        let (stream, _) = listener.accept().await?;
        if !authenticated(&stream, uid) {
            continue;
        }
        let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
            continue;
        };
        tokio::spawn(async move {
            let _slot = slot;
            let _ = tokio::time::timeout(Duration::from_secs(75), serve(stream)).await;
        });
    }
}

async fn serve(stream: UnixStream) -> Result<()> {
    let (reading, mut writing) = stream.into_split();
    let mut reading = BufReader::new(reading);
    let mut frame = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        (&mut reading).take(4097).read_until(b'\n', &mut frame),
    )
    .await
    .map_err(|_| Error::Refused("Incomplete workspace request."))??;
    if frame.len() > 4096 || frame.last() != Some(&b'\n') {
        return Err(Error::Refused("Invalid workspace request size."));
    }
    let request: LocalRequest = serde_json::from_slice(&frame)?;
    let result = match request {
        LocalRequest::DeliverPrivate {
            command,
            valid_until,
            checkout,
        } if matches!(command.action, Action::Start { .. }) => {
            deliver_private(command, valid_until, checkout, (&mut reading, &mut writing)).await
        }
        other => dispatch(other).await,
    };
    let reply = match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(_) => json!({"ok":false,"error":"Workspace lifecycle operation did not complete."}),
    };
    let mut bytes = serde_json::to_vec(&reply)?;
    if bytes.len() > 8192 {
        return Err(Error::Refused("Workspace reply exceeds its bound."));
    }
    bytes.push(b'\n');
    writing.write_all(&bytes).await?;
    Ok(())
}

async fn deliver_private(
    command: Command,
    valid_until: DateTime<Utc>,
    grant: CheckoutGrant,
    (reading, writing): (
        &mut (impl AsyncReadExt + Unpin),
        &mut (impl AsyncWriteExt + Unpin),
    ),
) -> Result<serde_json::Value> {
    let deadline = deadline(valid_until)?;
    let now = Utc::now();
    if !command.fits()
        || grant.repository_id.is_nil()
        || grant.expires_at <= now
        || grant.expires_at > now + TimeDelta::seconds(90)
    {
        return Err(Error::Refused(
            "Invalid private workspace start authorization.",
        ));
    }
    writing.write_all(CHECKOUT_READY).await?;
    let mut token = Zeroizing::new(Vec::with_capacity(MAX_CHECKOUT_TOKEN_BYTES + 1));
    tokio::time::timeout_at(
        deadline.min(tokio::time::Instant::now() + Duration::from_secs(5)),
        reading
            .take(MAX_CHECKOUT_TOKEN_BYTES as u64 + 1)
            .read_to_end(&mut token),
    )
    .await
    .map_err(|_| Error::Refused("The workspace checkout credential did not arrive."))??;
    let checkout = ManagedCredential::receive(grant, token)?;
    Ok(serde_json::to_value(
        super::deliver(command, deadline, Some(checkout)).await?,
    )?)
}

fn deadline(valid_until: DateTime<Utc>) -> Result<tokio::time::Instant> {
    let left = valid_until.signed_duration_since(Utc::now());
    if left <= TimeDelta::zero() || left > TimeDelta::seconds(15) {
        return Err(Error::Refused("Workspace delivery authorization expired."));
    }
    Ok(tokio::time::Instant::now()
        + left
            .to_std()
            .map_err(|_| Error::Refused("Invalid workspace deadline."))?)
}

async fn dispatch(request: LocalRequest) -> Result<serde_json::Value> {
    match request {
        LocalRequest::Ready => Ok(
            json!({"available": true, "capacity": super::capacity::observe().unwrap_or_default(), "privateCheckout": true}),
        ),
        LocalRequest::Inspect {
            workspace,
            principal,
        } => Ok(serde_json::to_value(super::inspect(workspace, principal)?)?),
        LocalRequest::Status {
            workspace,
            request,
            principal,
        } => Ok(serde_json::to_value(super::status(
            workspace, request, principal,
        )?)?),
        LocalRequest::Deliver {
            command,
            valid_until,
        } => {
            let receipt = super::deliver(command, deadline(valid_until)?, None).await?;
            Ok(serde_json::to_value(receipt)?)
        }
        LocalRequest::DeliverPrivate { .. } => Err(Error::Refused(
            "Only a start accepts a private checkout credential.",
        )),
    }
}
