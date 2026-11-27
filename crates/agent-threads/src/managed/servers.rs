//! Closed, generation-bound TCP listener observations, with no preview authority.
use super::{Offer, Poll};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_PORTS: usize = 16;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listeners {
    pub workspace: Uuid,
    pub generation: u64,
    pub ports: Vec<u16>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub workspaces: Vec<Offer>,
    pub servers: Vec<Listeners>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerPoll {
    pub poll: Poll,
    pub servers: Vec<Listeners>,
}

impl ServerPoll {
    #[must_use]
    pub fn fits(&self) -> bool {
        if !self.poll.fits() || self.servers.len() > self.poll.workspaces.len() {
            return false;
        }
        let mut ready: HashMap<_, _> = self
            .poll
            .workspaces
            .iter()
            .filter(|offer| offer.ready)
            .map(|offer| (offer.workspace, offer.generation))
            .collect();
        self.servers.iter().all(|row| {
            ready.remove(&row.workspace) == Some(row.generation)
                && row.ports.len() <= MAX_PORTS
                && row.ports.first().is_some_and(|port| *port > 0)
                && row.ports.windows(2).all(|pair| pair[0] < pair[1])
        })
    }
}
