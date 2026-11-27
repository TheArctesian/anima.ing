//! Who may use the database for a daemon now (K8 Section 6, MET-017).
//!
//! A request holds its turn only around its database work, never while a body arrives. A
//! request that gets no turn is answered `UNAVAILABLE` before it waits for a connection or a
//! row. The locks here are never held across an await.

use std::{
    collections::HashMap,
    hash::Hash,
    net::IpAddr,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::ids::DeviceId;

/// Well under the hub's pool of 10 connections, so the operator's requests and the checks find one.
pub(super) const DAEMON_REQUESTS: usize = 4;

/// The node's daemon sends one at a time, and an enrolment may run beside it (MET-012).
pub(super) const ADDRESS_REQUESTS: usize = 2;

#[derive(Clone)]
pub(super) struct Turns {
    requests: Arc<Semaphore>,
    wait: Duration,
    addresses: InFlight<IpAddr>,
    devices: InFlight<DeviceId>,
    polls: InFlight<DeviceId>,
}

impl Turns {
    pub(super) fn new(requests: usize, wait: Duration) -> Self {
        Self {
            requests: Arc::new(Semaphore::new(requests)),
            wait,
            addresses: InFlight::new(ADDRESS_REQUESTS),
            devices: InFlight::new(1),
            polls: InFlight::new(1),
        }
    }

    /// A turn to use the database, waited for in order; `None` when none came within the wait.
    pub(super) async fn request(&self) -> Option<OwnedSemaphorePermit> {
        let waited = Arc::clone(&self.requests).acquire_owned();
        tokio::time::timeout(self.wait, waited).await.ok()?.ok()
    }

    /// Limits an address until the credential is known, so one node with junk bearers cannot
    /// keep the hub busy for the others.
    pub(super) fn address(&self, address: IpAddr) -> Option<Turn<IpAddr>> {
        self.addresses.take(address)
    }

    /// One turn per device, so one token cannot queue requests on its device's rows.
    pub(super) fn device(&self, device: DeviceId) -> Option<Turn<DeviceId>> {
        self.devices.take(device)
    }
    pub(super) fn command_poll(&self, device: DeviceId) -> Option<Turn<DeviceId>> {
        self.polls.take(device)
    }
}

#[derive(Clone)]
struct InFlight<K> {
    counts: Arc<Mutex<HashMap<K, usize>>>,
    limit: usize,
}

impl<K: Copy + Eq + Hash> InFlight<K> {
    fn new(limit: usize) -> Self {
        Self {
            counts: Arc::default(),
            limit,
        }
    }

    fn take(&self, key: K) -> Option<Turn<K>> {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        let count = counts.entry(key).or_default();
        (*count < self.limit).then(|| {
            *count += 1;
            Turn {
                key,
                counts: Arc::clone(&self.counts),
            }
        })
    }
}

pub(super) struct Turn<K: Copy + Eq + Hash> {
    key: K,
    counts: Arc<Mutex<HashMap<K, usize>>>,
}

impl<K: Copy + Eq + Hash> Drop for Turn<K> {
    fn drop(&mut self) {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = counts.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&self.key);
            }
        }
    }
}
