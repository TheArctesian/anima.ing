//! Remote-only holder authentication. Legacy local frames retain their existing UID boundary.

use std::sync::Mutex;

use rustls::pki_types::CertificateDer;
use std::path::Path;
use uuid::Uuid;

use super::{Shared, locked, remote_code::Exchange};
use crate::threads::wire::remote_code::{ToHolder, ToRunner};

pub(super) struct Session {
    peer: Option<CertificateDer<'static>>,
    challenge: Option<Uuid>,
    authenticated: bool,
    pub(super) exchange: Option<Exchange>,
}

impl Session {
    pub(super) const fn new(peer: Option<CertificateDer<'static>>) -> Self {
        Self {
            peer,
            challenge: None,
            authenticated: false,
            exchange: None,
        }
    }

    async fn current(&self, dir: &Path) -> bool {
        let Some(root) = &self.peer else {
            return false;
        };
        crate::threads::holder_channel::public_root(dir)
            .await
            .as_ref()
            == Some(root)
    }

    pub(super) async fn receive(
        &mut self,
        shared: &Mutex<Shared>,
        frame: ToHolder,
    ) -> Option<ToHolder> {
        let dir = locked(shared).dir.clone();
        if !self.current(&dir).await {
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
