//! The flows of one UDP stream (task H41, SF-327). A source address of UDP can be false, and
//! nothing in its first datagram tells a false source from a real one. So a flow starts as
//! waiting, and becomes set up when its source sends again after the application answered it: a
//! real client does that within one round trip. A set-up flow keeps its place until it ends. A
//! new source that finds the waiting flows at their cap, or the table full, takes the place of
//! the oldest waiting flow, never of a set-up one. False sources of many networks then replace
//! only each other, and a real client that answers keeps its flow. Set-up flows take at most the
//! places that the waiting flows leave, so a new source always finds a waiting place.
//!
//! What stays (SF-327): a sender of false sources that sends again to each of them can set up
//! flows without seeing an answer, and then holds the set-up places until they are idle. The
//! caller limits new flows each second with a bucket whose burst is at least the cap of waiting
//! flows: once that bucket is empty, a waiting flow lives for at least the cap divided by the
//! rate, but a burst of false sources can replace each waiting flow at once. No I/O and no clock:
//! the order of the flows is a counter.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use crate::limits::source_network;

/// The caps of the flows of one stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowCaps {
    /// Flows in all, waiting and set up.
    pub total: usize,
    /// Flows of one source network ([`source_network`]), waiting and set up.
    pub per_network: usize,
    /// Waiting flows.
    pub waiting: usize,
    /// Set-up flows: at most `total` less `waiting`, so a new source always finds a waiting place.
    pub set_up: usize,
}

/// A flow that [`FlowTable::open`] made: its number, and the handle of a waiting flow that it
/// took the place of, which the caller drops to end that flow.
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

/// The flows of one stream, by source address, each with a handle `T` (in the proxy, the sender
/// of its datagrams; when the table drops it, the flow ends).
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

    /// The handle of the flow of `source`, if it has one.
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

    /// Removes the waiting flow opened first and gives its handle; `None` when no flow waits.
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
