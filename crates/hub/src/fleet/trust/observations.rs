//! Host keys and certificates that connection tests saw and the operator has not accepted, kept
//! so that test, save, then accept works for a new device (SEC-006, SEC-007).

use std::{collections::HashMap, net::IpAddr};

use chrono::{DateTime, TimeDelta, Utc};

pub(in crate::fleet::trust) const LIFETIME: TimeDelta = TimeDelta::minutes(15);

/// Bounds memory even when a signed-in operator tests many hosts in a loop.
pub(in crate::fleet::trust) const MAX_OBSERVATIONS: usize = 256;

/// A host and a port in one spelling. A final dot is kept: `pve-01.lan.` and `pve-01.lan` can
/// name different hosts through a search domain.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Endpoint {
    host: String,
    port: u16,
}

impl Endpoint {
    pub(crate) fn new(host: &str, port: u16) -> Self {
        let host = host.trim();
        let host = host
            .parse::<IpAddr>()
            .map_or_else(|_| host.to_ascii_lowercase(), |ip| ip.to_string());
        Self { host, port }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Observation<T> {
    pub(crate) presented: T,
    pub(crate) first_seen_at: DateTime<Utc>,
    pub(crate) last_seen_at: DateTime<Utc>,
}

impl<T> Observation<T> {
    fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.last_seen_at + LIFETIME
    }
}

#[derive(Debug)]
pub(in crate::fleet::trust) struct Observations<T>(HashMap<Endpoint, Observation<T>>);

impl<T> Default for Observations<T> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<T: Clone + PartialEq> Observations<T> {
    pub(in crate::fleet::trust) fn record(
        &mut self,
        endpoint: Endpoint,
        presented: T,
        now: DateTime<Utc>,
    ) -> Observation<T> {
        self.0.retain(|_, seen| !seen.is_expired(now));
        let first_seen_at = match self.0.get(&endpoint) {
            Some(seen) if seen.presented == presented => seen.first_seen_at,
            _ => now,
        };
        if !self.0.contains_key(&endpoint) && self.0.len() >= MAX_OBSERVATIONS {
            self.forget_oldest();
        }
        let observation = Observation {
            presented,
            first_seen_at,
            last_seen_at: now,
        };
        self.0.insert(endpoint, observation.clone());
        observation
    }

    pub(in crate::fleet::trust) fn find(
        &self,
        endpoint: &Endpoint,
        now: DateTime<Utc>,
    ) -> Option<Observation<T>> {
        self.0
            .get(endpoint)
            .filter(|seen| !seen.is_expired(now))
            .cloned()
    }

    fn forget_oldest(&mut self) {
        let oldest = self
            .0
            .iter()
            .min_by_key(|(_, seen)| seen.last_seen_at)
            .map(|(endpoint, _)| endpoint.clone());
        if let Some(endpoint) = oldest {
            self.0.remove(&endpoint);
        }
    }

    #[cfg(test)]
    pub(in crate::fleet::trust) fn len(&self) -> usize {
        self.0.len()
    }
}
