//! A bounded live channel for the authenticated link; no challenge is persisted.
use super::{Error, Result, provider_login};
use agent_threads::managed::provider_login::{Login, MAX_REQUEST, SOCKET};
use std::{collections::HashMap, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Mutex, Semaphore},
};
use uuid::Uuid;

type Sessions = Arc<Mutex<HashMap<Uuid, chrono::DateTime<chrono::Utc>>>>;

pub(super) async fn run(uid: u32) -> Result<()> {
    match std::fs::remove_file(SOCKET) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let listener = UnixListener::bind(SOCKET)?;
    std::fs::set_permissions(SOCKET, std::fs::Permissions::from_mode(0o660))?;
    let slots = Arc::new(Semaphore::new(4));
    let sessions = Arc::new(Mutex::new(HashMap::new()));
    loop {
        let (stream, _) = listener.accept().await?;
        if !stream.peer_cred().is_ok_and(|peer| peer.uid() == uid) {
            continue;
        }
        let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
            continue;
        };
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            let _slot = slot;
            let _ = tokio::time::timeout(Duration::from_secs(605), serve(stream, sessions)).await;
        });
    }
}

async fn serve(stream: UnixStream, sessions: Sessions) -> Result<()> {
    let (reading, writing) = stream.into_split();
    let mut reader = BufReader::new(reading).take((MAX_REQUEST + 1) as u64);
    let mut frame = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), reader.read_until(b'\n', &mut frame))
        .await
        .map_err(|_| Error::Refused("Incomplete provider request."))??;
    if frame.len() > MAX_REQUEST || frame.last() != Some(&b'\n') {
        return Err(Error::Refused("Invalid provider request size."));
    }
    let login: Login = serde_json::from_slice(&frame)?;
    let now = chrono::Utc::now();
    if !login.valid(now) {
        return Err(Error::Refused("Provider sign-in authorization expired."));
    }
    {
        let mut seen = sessions.lock().await;
        seen.retain(|_, deadline| *deadline > now);
        if seen.contains_key(&login.session) || seen.len() >= 128 {
            return Err(Error::Refused(
                "Provider sign-in request cannot be replayed.",
            ));
        }
        seen.insert(login.session, login.expires_at);
    }
    provider_login::run(login, reader.into_inner(), writing).await
}
