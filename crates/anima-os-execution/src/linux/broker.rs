//! The node daemon receives a closed local API, never the root owner's CLI surface.
use super::{Error, Result, control, workspace};
use agent_threads::managed::{BROKER_SOCKET, LocalRequest, Offer, servers::Inventory};
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Semaphore,
};

pub(super) async fn run() -> Result<()> {
    let daemon = nix::unistd::User::from_name("anima-daemon")
        .map_err(|_| Error::Refused("The enrolled daemon account is unavailable."))?
        .ok_or(Error::Refused(
            "The enrolled daemon account is unavailable.",
        ))?;
    match std::fs::remove_file(BROKER_SOCKET) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let listener = UnixListener::bind(BROKER_SOCKET)?;
    std::fs::set_permissions(BROKER_SOCKET, std::fs::Permissions::from_mode(0o660))?;
    tokio::try_join!(
        commands(listener, daemon.uid.as_raw()),
        super::preview_broker::run(daemon.uid.as_raw()),
        super::provider_broker::run(daemon.uid.as_raw())
    )?;
    Ok(())
}

async fn commands(listener: UnixListener, uid: u32) -> Result<()> {
    let slots = Arc::new(Semaphore::new(8));
    loop {
        let (stream, _) = listener.accept().await?;
        if !stream.peer_cred().is_ok_and(|peer| peer.uid() == uid) {
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
    let mut frame = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        BufReader::new(reading)
            .take(65537)
            .read_until(b'\n', &mut frame),
    )
    .await
    .map_err(|_| Error::Refused("Incomplete broker request."))??;
    if frame.len() > 65536 || frame.last() != Some(&b'\n') {
        return Err(Error::Refused("Invalid broker request size."));
    }
    let request: LocalRequest = serde_json::from_slice(&frame)?;
    let result = match request {
        LocalRequest::Offers => offers(false)
            .await
            .and_then(|inventory| Ok(serde_json::to_value(inventory.workspaces)?)),
        LocalRequest::OffersWithServers => offers(true)
            .await
            .and_then(|inventory| Ok(serde_json::to_value(inventory)?)),
        LocalRequest::ClosePreview {
            endpoint,
            revision,
            principal,
        } => super::previews::close_for(endpoint, revision, principal),
        LocalRequest::Snapshot {
            workspace,
            generation,
            thread,
            after,
        } => control::snapshot(workspace, generation, thread, after).await,
        LocalRequest::Status {
            workspace,
            request,
            principal,
        } => control::status(workspace, request, principal).await,
        LocalRequest::Deliver {
            command,
            valid_until,
        } => deliver(*command, valid_until).await,
    };
    let reply = match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(_) => json!({"ok":false,"error":"Managed Code broker operation did not complete."}),
    };
    let mut bytes = serde_json::to_vec(&reply)?;
    if bytes.len() > 1_100_000 {
        return Err(Error::Refused("Broker response exceeded its limit."));
    }
    bytes.push(b'\n');
    writing.write_all(&bytes).await?;
    Ok(())
}

async fn offers(detect_servers: bool) -> Result<Inventory> {
    let inventory = workspace::inventory()?;
    let entries = inventory["entries"]
        .as_array()
        .ok_or(Error::Refused("Workspace inventory unavailable."))?;
    if inventory["truncated"] != false || entries.len() > 128 {
        return Err(Error::Refused(
            "Workspace offer inventory exceeded its limit.",
        ));
    }
    let mut offers = Vec::new();
    let mut probes = tokio::task::JoinSet::new();
    let slots = Arc::new(Semaphore::new(8));
    let admission_ready = anima_os_maintenance::recovery::ready().unwrap_or(false);
    for entry in entries {
        let workspace = serde_json::from_value(entry["workspace"].clone())?;
        let generation = entry["generation"]
            .as_u64()
            .ok_or(Error::Refused("Invalid workspace generation."))?;
        if generation == 0 {
            continue;
        }
        let index = offers.len();
        offers.push(Offer {
            workspace,
            generation,
            ready: false,
        });
        if admission_ready && entry["phase"] == "running" {
            let slots = Arc::clone(&slots);
            probes.spawn(async move {
                let _slot = slots.acquire_owned().await;
                let ready = tokio::time::timeout(
                    Duration::from_secs(2),
                    workspace::connection(workspace, generation),
                )
                .await
                .ok()
                .and_then(Result::ok);
                (index, ready)
            });
        }
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut listeners = Vec::new();
    while let Ok(Some(result)) = tokio::time::timeout_at(deadline, probes.join_next()).await {
        if let Ok((index, Some(container))) = result {
            let offer = &mut offers[index];
            offer.ready = true;
            if detect_servers {
                listeners.push((offer.workspace, offer.generation, container));
            }
        }
    }
    probes.abort_all();
    Ok(Inventory {
        workspaces: offers,
        servers: workspace::servers::observe(listeners).await,
    })
}

async fn deliver(
    command: control::Command,
    valid_until: chrono::DateTime<chrono::Utc>,
) -> Result<Value> {
    let left = valid_until.signed_duration_since(chrono::Utc::now());
    if left <= chrono::TimeDelta::zero() || left > chrono::TimeDelta::seconds(15) {
        Err(Error::Refused(
            "Managed Code delivery authorization expired or exceeds its lifetime.",
        ))
    } else {
        let remaining = left
            .to_std()
            .map_err(|_| Error::Refused("Invalid delivery deadline."))?;
        control::authorized(command, Some(tokio::time::Instant::now() + remaining)).await
    }
}
