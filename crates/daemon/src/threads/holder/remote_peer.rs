//! Remote-only holder authentication. Legacy local frames retain their existing UID boundary.

use std::sync::Mutex;

use tokio::net::UnixStream;
use uuid::Uuid;

use super::{Shared, locked, remote_code::Exchange};
use crate::threads::wire::remote_code::{ToHolder, ToRunner};

pub(super) struct Session {
    peer: Option<Peer>,
    challenge: Option<Uuid>,
    authenticated: bool,
    pub(super) exchange: Option<Exchange>,
}

impl Session {
    pub(super) const fn new(peer: Option<Peer>) -> Self {
        Self {
            peer,
            challenge: None,
            authenticated: false,
            exchange: None,
        }
    }

    pub(super) async fn receive(
        &mut self,
        shared: &Mutex<Shared>,
        frame: ToHolder,
    ) -> Option<ToHolder> {
        if !self.peer.as_ref()?.is_daemon().await {
            self.authenticated = false;
            self.challenge = None;
            self.exchange = None;
            return None;
        }
        match frame {
            ToHolder::Open => {
                self.authenticated = false;
                self.exchange = None;
                let nonce = random_nonce()?;
                self.challenge = Some(nonce);
                notify(shared, &ToRunner::Challenge { nonce });
                None
            }
            ToHolder::Prove { nonce } => {
                self.exchange = None;
                self.authenticated = self.challenge.take() == Some(nonce);
                if self.authenticated {
                    notify(shared, &ToRunner::Ready);
                }
                None
            }
            action => self.authenticated.then_some(action),
        }
    }
}

fn random_nonce() -> Option<Uuid> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(Uuid::from_bytes(bytes))
}

fn notify(shared: &Mutex<Shared>, response: &ToRunner) {
    if let Some(event) = response.event() {
        locked(shared).send(&event);
    }
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(in crate::threads) struct Peer(crate::threads::tree::identity::Process);

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
impl Peer {
    pub(in crate::threads) fn of(stream: &UnixStream) -> Option<Self> {
        let credentials = stream.peer_cred().ok()?;
        if credentials.uid() != rustix::process::getuid().as_raw() {
            return None;
        }
        let peer = Self(crate::threads::tree::identity::Process::peer(
            stream,
            credentials.pid(),
        )?);
        peer.matches_executable().then_some(peer)
    }

    fn matches_executable(&self) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        let Some((own, remote)) = (|| {
            if !self.0.live() {
                return None;
            }
            let own = std::fs::metadata("/proc/self/exe").ok()?;
            let remote = std::fs::metadata(format!("/proc/{}/exe", self.0.identity().pid)).ok()?;
            Some((own, remote))
        })() else {
            return false;
        };
        own.dev() == remote.dev() && own.ino() == remote.ino() && self.0.live()
    }

    pub(in crate::threads) async fn is_daemon(&self) -> bool {
        let peer = self.0.clone();
        tokio::task::spawn_blocking(move || Self(peer).matches_executable())
            .await
            .unwrap_or(false)
    }
}

// On macOS LOCAL_PEERTOKEN looks up the peer's last numeric PID at query time.
// It cannot prove that a live code signature belongs to the original socket peer.
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub(in crate::threads) enum Peer {}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
impl Peer {
    pub(in crate::threads) const fn of(_: &UnixStream) -> Option<Self> {
        None
    }

    pub(in crate::threads) const fn is_daemon(&self) -> std::future::Ready<bool> {
        match *self {}
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn th32_remote_peer_macos_requires_stable_socket_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("peer.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let client = UnixStream::connect(&socket).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        assert!(Peer::of(&server).is_none());
        drop(client);
    }
}
