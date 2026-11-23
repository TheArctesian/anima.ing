//! Files tunnel tickets, kept apart from session tickets so neither opens the other's tunnel (K4 Sections 1 and 9, WP-27 D-5).
//! Only the SHA-256 of a token is held, in memory (LOG-007).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use chrono::{DateTime, Utc};

use crate::{
    fleet::{
        events::{ClientAddr, Operator},
        sessions::{
            Refused,
            tickets::{TicketHash, hash_of},
        },
    },
    ids::{DeviceId, SessionId},
};

/// Bounds what a client asking in a loop can hold; expired tickets are dropped as new ones are issued.
pub(super) const MAX_PENDING: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FilesTicket {
    pub(super) operator: Operator,
    pub(super) client: ClientAddr,
    pub(super) session: SessionId,
    pub(super) device: DeviceId,
    pub(super) expires_at: DateTime<Utc>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("no files ticket can be issued now")]
pub(super) struct Full;

#[derive(Clone, Default)]
pub(in crate::fleet::sessions) struct Tickets(Arc<Mutex<HashMap<TicketHash, FilesTicket>>>);

impl Tickets {
    /// Takes over a poisoned lock: no critical section here panics between two writes.
    fn held(&self) -> MutexGuard<'_, HashMap<TicketHash, FilesTicket>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn issue(
        &self,
        hash: TicketHash,
        ticket: FilesTicket,
        now: DateTime<Utc>,
    ) -> Result<(), Full> {
        let mut held = self.held();
        held.retain(|_, pending| now < pending.expires_at);
        if held.len() >= MAX_PENDING {
            return Err(Full);
        }
        held.insert(hash, ticket);
        Ok(())
    }

    /// Removes the token first, valid or not, so it works at most once (K4 Section 1).
    /// Every refusal is [`Refused`], with no detail.
    pub(super) fn redeem(
        &self,
        presented: &str,
        client: ClientAddr,
        now: DateTime<Utc>,
    ) -> Result<FilesTicket, Refused> {
        let hash = hash_of(presented).ok_or(Refused)?;
        let ticket = self.held().remove(&hash).ok_or(Refused)?;
        if now >= ticket.expires_at || ticket.client != client {
            return Err(Refused);
        }
        Ok(ticket)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.held().len()
    }
}
