//! Isolated connection budget for preview streams; only the enrolled transport UID connects.
use super::{Error, Result, previews, transport};
use agent_threads::managed::preview::{MAX_REQUEST, Open, SOCKET};
use std::{os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Semaphore,
};

pub(super) async fn run(uid: u32) -> Result<()> {
    match std::fs::remove_file(SOCKET) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let listener = UnixListener::bind(SOCKET)?;
    std::fs::set_permissions(SOCKET, std::fs::Permissions::from_mode(0o660))?;
    let slots = Arc::new(Semaphore::new(16));
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
            let _ = tokio::time::timeout(Duration::from_secs(605), serve(stream)).await;
        });
    }
}

async fn serve(stream: UnixStream) -> Result<()> {
    let (reading, writing) = stream.into_split();
    // Retain buffered bytes: a client may send the first HTTP bytes with its opening frame.
    let mut reader = BufReader::new(reading).take((MAX_REQUEST + 1) as u64);
    let mut line = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), reader.read_until(b'\n', &mut line))
        .await
        .map_err(|_| Error::Refused("Incomplete preview request."))??;
    if line.len() > MAX_REQUEST || line.last() != Some(&b'\n') {
        return Err(Error::Refused("Invalid preview request size."));
    }
    let request: Open = serde_json::from_slice(&line)?;
    let remaining = request
        .valid_until
        .signed_duration_since(chrono::Utc::now());
    if remaining <= chrono::TimeDelta::zero() || remaining > chrono::TimeDelta::seconds(15) {
        return Err(Error::Refused("Preview authorization expired."));
    }
    let deadline = tokio::time::Instant::now()
        + remaining
            .to_std()
            .map_err(|_| Error::Refused("Invalid preview deadline."))?;
    let binding = request.binding;
    previews::open(binding.clone()).await?;
    if tokio::time::Instant::now() >= deadline {
        return Err(Error::Refused("Preview authorization expired."));
    }
    let lease = previews::resolve(binding.endpoint, 1, binding.principal)?;
    transport::preview_to(lease, reader.into_inner(), writing).await
}
