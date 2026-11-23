//! The flows of one UDP stream (SF-327): a flow waits until its source sends again after an answer,
//! and a new source replaces only the oldest waiting flow, never a set-up one.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use crate::limits::source_network;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowCaps {
    pub total: usize,
    /// Flows of one source network ([`source_network`]), waiting and set up.
    pub per_network: usize,
    pub waiting: usize,
    /// Set-up flows: at most `total` less `waiting`, so a new source always finds a waiting place.
    pub set_up: usize,
}

/// A flow that [`FlowTable::open`] made, and the waiting flow it replaced, which the caller drops.
#[derive(Debug)]
pub struct Opened<T> {
    pub id: u64,
    pub replaced: Option<T>,
}

#[derive(Debug)]
struct Flow<T> {
    id: u64,
    handle: T,
    set_up: bool,
}

/// The flows of one stream by source address; dropping a flow's handle `T` ends the flow.
#[derive(Debug)]
pub struct FlowTable<T> {
    caps: FlowCaps,
    flows: HashMap<SocketAddr, Flow<T>>,
    next: u64,
}

impl<T> FlowTable<T> {
    #[must_use]
    pub fn new(caps: FlowCaps) -> Self {
        Self {
            caps,
            flows: HashMap::new(),
            next: 0,
        }
    }

    #[must_use]
    pub fn get(&self, source: SocketAddr) -> Option<&T> {
        self.flows.get(&source).map(|flow| &flow.handle)
    }

    /// A new waiting flow of `source` with `handle`; `None` when the network of the source is at
    /// its cap, or when each place is a set-up flow.
    pub fn open(&mut self, source: SocketAddr, handle: T) -> Option<Opened<T>> {
        if self.flows.contains_key(&source) || self.of_network(source.ip()) >= self.caps.per_network
        {
            return None;
        }
        let waiting = self.flows.values().filter(|flow| !flow.set_up).count();
        let full = self.flows.len() >= self.caps.total || waiting >= self.caps.waiting;
        let replaced = if full {
            Some(self.take_oldest_waiting()?)
        } else {
            None
        };
        let id = self.next;
        self.next += 1;
        self.flows.insert(
            source,
            Flow {
                id,
                handle,
                set_up: false,
            },
        );
        Some(Opened { id, replaced })
    }

    /// Marks the flow `id` of `source` set up; `false` when it is no longer in the table, or when
    /// the set-up flows are at their cap (it stays waiting).
    pub fn set_up(&mut self, source: SocketAddr, id: u64) -> bool {
        let set_up = self.flows.values().filter(|flow| flow.set_up).count();
        let room = set_up
            < self
                .caps
                .set_up
                .min(self.caps.total.saturating_sub(self.caps.waiting));
        match self.flows.get_mut(&source) {
            Some(flow) if flow.id == id && (flow.set_up || room) => {
                flow.set_up = true;
                true
            }
            _ => false,
        }
    }

    /// Removes the flow `id` of `source`, and not a later flow of the same source.
    pub fn close(&mut self, source: SocketAddr, id: u64) {
        if self.flows.get(&source).is_some_and(|flow| flow.id == id) {
            self.flows.remove(&source);
        }
    }

    pub fn clear(&mut self) {
        self.flows.clear();
    }

    /// The flows in the table, and how many of them are set up.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        let set_up = self.flows.values().filter(|flow| flow.set_up).count();
        (self.flows.len(), set_up)
    }

    fn of_network(&self, source: IpAddr) -> usize {
        let network = source_network(source);
        self.flows
            .keys()
            .filter(|known| source_network(known.ip()) == network)
            .count()
    }

    fn take_oldest_waiting(&mut self) -> Option<T> {
        let oldest = self
            .flows
            .iter()
            .filter(|(_, flow)| !flow.set_up)
            .min_by_key(|(_, flow)| flow.id)
            .map(|(source, _)| *source)?;
        self.flows.remove(&oldest).map(|flow| flow.handle)
    }
}
