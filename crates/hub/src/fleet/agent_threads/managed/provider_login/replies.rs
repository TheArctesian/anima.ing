//! Ephemeral human reply handles. Dropping the live stream removes its handle.
#[cfg(test)]
mod tests;
use crate::ids::DeviceId;
use agent_threads::managed::provider_login::Login;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};
use tokio::{sync::mpsc, time::Instant};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Default)]
pub struct Replies(Arc<Mutex<HashMap<Uuid, Open>>>);

struct Open {
    device: DeviceId,
    login: Login,
    deadline: Instant,
    ready: bool,
    send: Option<mpsc::Sender<Zeroizing<String>>>,
}

pub(super) struct Lease {
    book: Replies,
    session: Uuid,
    pub receive: mpsc::Receiver<Zeroizing<String>>,
}

impl Replies {
    pub(super) fn open(&self, device: DeviceId, login: &Login, deadline: Instant) -> Option<Lease> {
        let mut book = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        book.retain(|_, open| open.deadline > now);
        if deadline <= now
            || book.len() >= 64
            || book.contains_key(&login.session)
            || book
                .values()
                .filter(|open| open.login.principal == login.principal)
                .count()
                >= 4
        {
            return None;
        }
        let (send, receive) = mpsc::channel(1);
        book.insert(
            login.session,
            Open {
                device,
                login: login.clone(),
                deadline,
                ready: false,
                send: Some(send),
            },
        );
        Some(Lease {
            book: self.clone(),
            session: login.session,
            receive,
        })
    }

    pub(super) fn binding(&self, session: Uuid, principal: Uuid) -> Option<(DeviceId, Login)> {
        let book = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let open = book.get(&session)?;
        (open.ready
            && open.send.is_some()
            && open.deadline > Instant::now()
            && open.login.principal == principal)
            .then(|| (open.device, open.login.clone()))
    }

    pub(super) fn submit(&self, session: Uuid, principal: Uuid, code: Zeroizing<String>) -> bool {
        let mut book = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(open) = book.get_mut(&session) else {
            return false;
        };
        if !open.ready || open.deadline <= Instant::now() || open.login.principal != principal {
            return false;
        }
        open.send
            .take()
            .is_some_and(|send| send.try_send(code).is_ok())
    }
}

impl Lease {
    pub(super) fn ready(&self) -> bool {
        let mut book = self.book.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(open) = book.get_mut(&self.session) else {
            return false;
        };
        if open.deadline <= Instant::now() {
            return false;
        }
        open.ready = true;
        true
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.book
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.session);
    }
}
