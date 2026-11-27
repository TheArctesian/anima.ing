//! The daemon may request bounded lifecycle operations, never owner policy changes.
use super::{
    Error, Request, Result, artifact, authorized, checkout, cleanup, inventory, registration,
    retirement, store,
};
use daemon_core::source_jobs::{Action, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, SOCKET};
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Semaphore,
};

pub(in crate::linux) async fn run() -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused("Source lifecycle requires the root owner."));
    }
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o077));
    let user = nix::unistd::User::from_name("anima-daemon")
        .map_err(std::io::Error::from)?
        .ok_or(Error::Refused(
            "The enrolled daemon account is unavailable.",
        ))?;
    match std::fs::remove_file(SOCKET) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let listener = UnixListener::bind(SOCKET)?;
    std::fs::set_permissions(SOCKET, std::fs::Permissions::from_mode(0o660))?;
    let slots = Arc::new(Semaphore::new(4));
    loop {
        let (stream, _) = listener.accept().await?;
        if !stream
            .peer_cred()
            .is_ok_and(|peer| peer.uid() == user.uid.as_raw())
        {
            continue;
        }
        let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
            continue;
        };
        tokio::spawn(async move {
            let _slot = slot;
            let _ = serve(stream).await;
        });
    }
}

async fn serve(stream: UnixStream) -> Result<()> {
    let (reading, mut writing) = stream.into_split();
    let mut reading = BufReader::new(reading);
    let mut frame = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        (&mut reading)
            .take(MAX_REQUEST_BYTES as u64 + 1)
            .read_until(b'\n', &mut frame),
    )
    .await
    .map_err(|_| Error::Refused("Incomplete source-job request."))??;
    if frame.len() > MAX_REQUEST_BYTES || frame.last() != Some(&b'\n') {
        return Err(Error::Refused("Invalid source-job request size."));
    }
    let request: daemon_core::source_jobs::Request = serde_json::from_slice(&frame)?;
    if let Action::Checkout {
        identity,
        expires_at,
    } = request.action
    {
        if !reading.buffer().is_empty() {
            return Err(Error::Refused(
                "Wait for the checkout owner before sending the credential.",
            ));
        }
        let mut reading = reading.into_inner();
        let result = checkout::relay(
            (request.device, &request.origin),
            (identity, expires_at),
            (&mut reading, &mut writing),
        )
        .await;
        return write_reply(result, writing).await;
    }
    if matches!(request.action, Action::Export { .. }) {
        return artifact::send(request, writing).await;
    }
    tokio::time::timeout(Duration::from_secs(75), reply(request, writing))
        .await
        .map_err(|_| Error::Refused("Source-job request timed out."))?
}

async fn reply(
    request: daemon_core::source_jobs::Request,
    writing: tokio::net::unix::OwnedWriteHalf,
) -> Result<()> {
    write_reply(dispatch(request).await, writing).await
}

async fn write_reply(
    outcome: Result<Value>,
    mut writing: tokio::net::unix::OwnedWriteHalf,
) -> Result<()> {
    let reply = match outcome {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(_) => json!({"ok":false,"error":"Source-job operation did not complete."}),
    };
    let mut bytes = serde_json::to_vec(&reply)?;
    if bytes.len() > MAX_REPLY_BYTES {
        return Err(Error::Refused("Source-job reply exceeded its bound."));
    }
    bytes.push(b'\n');
    writing.write_all(&bytes).await?;
    Ok(())
}

async fn dispatch(request: daemon_core::source_jobs::Request) -> Result<Value> {
    let daemon_core::source_jobs::Request {
        device,
        origin,
        action,
    } = request;
    let (execution, request) = match action {
        Action::Release { execution, receipt } => {
            return cleanup::queue(execution, (device, &origin), receipt);
        }
        Action::Retire { identity } => {
            return retirement::forget((device, &origin), identity);
        }
        Action::Export { .. } => return Err(Error::Refused("Artifact export requires streaming.")),
        Action::Checkout { .. } => {
            return Err(Error::Refused(
                "Private checkout requires the credential relay.",
            ));
        }
        Action::Capacity => return Request::Capacity.execute().await,
        Action::List => {
            let _lock = store::lock()?;
            return inventory::list(device, &origin, true);
        }
        Action::Register { execution, item } => {
            let _lock = store::lock()?;
            return registration::register(execution, (device, origin), &item, true);
        }
        Action::Start { execution } => (execution, Request::Start { execution }),
        Action::Status { execution } => (execution, Request::Status { execution }),
        Action::Renew { execution, lease } => (execution, Request::Renew { execution, lease }),
        Action::Cancel { execution } => (execution, Request::Cancel { execution }),
    };
    authorized(execution, device, &origin)?;
    request.execute().await
}
