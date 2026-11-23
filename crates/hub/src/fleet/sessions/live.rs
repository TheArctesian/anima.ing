//! What the hub holds in memory about sessions (D-1, R-032, SEC-006, SEC-007, RA-034).

mod changes;
mod tunnel;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::sync::watch;

pub(super) use self::tunnel::{Closing, FilesRelay};
pub use self::tunnel::{EndCause, Tunnel};
use self::{
    changes::Changes,
    tunnel::{EndSignal, signal},
};
use super::{
    end::Actor,
    files,
    model::EndReason,
    tickets::{PendingTicket, Presented, TicketHash, hash_of},
};
use crate::{
    fleet::{targets::AddressPolicy, trust::Sightings},
    ids::SessionId,
};

/// Bounds what one operator can hold by starting sessions in a loop; spent tickets count too.
pub(super) const MAX_PENDING: usize = 128;

/// How long `endSession` waits by default for an open tunnel to end its row (D-10).
pub(super) const TUNNEL_END_WAIT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct State {
    tickets: HashMap<TicketHash, PendingTicket>,
    /// Refused tickets, kept without their token until they expire so the sweep ends their rows.
    spent: HashMap<SessionId, PendingTicket>,
    tunnels: HashMap<SessionId, EndSignal>,
    files_relays: HashMap<u64, EndSignal>,
    next_files_relay: u64,
    shutting_down: bool,
}

impl State {
    fn refuses_tickets(&self) -> bool {
        self.shutting_down || self.tickets.len() + self.spent.len() >= MAX_PENDING
    }

    fn withdraw(&mut self, session: SessionId) -> Option<PendingTicket> {
        if let Some(spent) = self.spent.remove(&session) {
            return Some(spent);
        }
        let hash = self
            .tickets
            .iter()
            .find_map(|(hash, ticket)| (ticket.session == session).then_some(*hash))?;
        self.tickets.remove(&hash)
    }

    fn holds_ticket(&self, session: SessionId) -> bool {
        self.tickets
            .values()
            .any(|ticket| ticket.session == session)
    }
}

/// `Live::default()` has no deny list; the hub applies `ANIMA_PROBE_DENY` with [`Live::denying`].
#[derive(Clone)]
pub struct Live {
    state: Arc<Mutex<State>>,
    /// Has its own locks, so a connection test never waits on a ticket redemption.
    sightings: Sightings,
    tunnel_end_wait: Duration,
    /// The one target rule every session connection applies (R-030, R-032).
    policy: Arc<AddressPolicy>,
    files: files::Tickets,
    changes: Changes,
}

impl Default for Live {
    fn default() -> Self {
        Self::with_tunnel_end_wait(TUNNEL_END_WAIT)
    }
}

/// Carries no detail, so the HTTP 403 cannot tell refusal causes apart (K4 §2).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("the ticket is not valid")]
pub struct Refused;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("no ticket can be issued now")]
pub(super) struct Unavailable;

pub struct Redeemed {
    pub ticket: PendingTicket,
    pub tunnel: Tunnel,
}

pub(super) enum Ending {
    /// An open tunnel was told to end; it writes the row's end.
    Signalled(Closing),
    /// A pending or spent ticket was withdrawn; the caller ends the row.
    Withdrawn(PendingTicket),
    Neither,
}

impl Live {
    #[must_use]
    pub fn with_tunnel_end_wait(wait: Duration) -> Self {
        Self {
            state: Arc::default(),
            sightings: Sightings::default(),
            tunnel_end_wait: wait,
            policy: Arc::default(),
            files: files::Tickets::default(),
            changes: Changes::default(),
        }
    }

    /// Every session connection also refuses what `policy` refuses; it replaces any earlier policy.
    #[must_use]
    pub fn denying(self, policy: AddressPolicy) -> Self {
        #[cfg(any(test, feature = "test-support"))]
        let policy = if self.policy.allows_loopback() {
            policy.allowing_loopback()
        } else {
            policy
        };
        Self {
            policy: Arc::new(policy),
            ..self
        }
    }

    /// Tests only: every other refused address, and a denied loopback, stays refused (R-030, R-032).
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn reaching_loopback_for_tests(self) -> Self {
        Self {
            policy: Arc::new(Arc::unwrap_or_clone(self.policy).allowing_loopback()),
            ..self
        }
    }

    #[must_use]
    pub const fn sightings(&self) -> &Sightings {
        &self.sightings
    }

    pub(super) const fn tunnel_end_wait(&self) -> Duration {
        self.tunnel_end_wait
    }

    pub(super) const fn files(&self) -> &files::Tickets {
        &self.files
    }

    pub(super) fn has_tunnel(&self, session: SessionId) -> bool {
        self.state().tunnels.contains_key(&session)
    }

    /// Decides under one lock, so shutdown cannot slip between the tunnel check and the registration.
    pub(super) fn files_relay(
        &self,
        session: SessionId,
    ) -> Option<(watch::Receiver<Option<EndCause>>, FilesRelay)> {
        let mut state = self.state();
        if state.shutting_down {
            return None;
        }
        let tunnel_end = state.tunnels.get(&session)?.subscribe();
        let id = state.next_files_relay;
        state.next_files_relay = id.wrapping_add(1);
        let (registration, held) = watch::channel(None);
        state.files_relays.insert(id, registration);
        drop(state);
        let relay = FilesRelay {
            id,
            _held: held,
            live: self.clone(),
        };
        Some((tunnel_end, relay))
    }

    pub(super) fn policy(&self) -> &AddressPolicy {
        &self.policy
    }

    /// Takes over a poisoned lock: no critical section here panics between two writes.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn is_shutting_down(&self) -> bool {
        self.state().shutting_down
    }

    /// Advisory: a start asks before it wakes a device, and [`Live::issue`] still decides.
    pub(super) fn is_full(&self) -> bool {
        self.state().refuses_tickets()
    }

    pub(super) fn issue(&self, hash: TicketHash, ticket: PendingTicket) -> Result<(), Unavailable> {
        let mut state = self.state();
        if state.refuses_tickets() {
            return Err(Unavailable);
        }
        state.tickets.insert(hash, ticket);
        Ok(())
    }

    /// Removes the token first, valid or not, so it works at most once (K4 §1).
    /// An accepted ticket becomes a tunnel under the same lock, so `end_by` always finds it.
    ///
    /// # Errors
    /// `Refused`, with no detail, in every refused case.
    pub fn redeem(
        &self,
        presented: Presented<'_>,
        now: DateTime<Utc>,
    ) -> Result<Redeemed, Refused> {
        let hash = hash_of(presented.token).ok_or(Refused)?;
        let mut state = self.state();
        let ticket = state.tickets.remove(&hash).ok_or(Refused)?;
        if !ticket.admits(&presented, now)
            || state.shutting_down
            || state.tunnels.contains_key(&ticket.session)
        {
            state.spent.insert(ticket.session, ticket);
            return Err(Refused);
        }
        let (end, cause) = watch::channel(None);
        state.tunnels.insert(ticket.session, end);
        drop(state);
        let tunnel = Tunnel {
            session: ticket.session,
            cause,
            live: self.clone(),
        };
        Ok(Redeemed { ticket, tunnel })
    }

    /// Decides under one lock, so a redemption cannot slip between the two cases (D-10).
    pub(super) fn end_by(&self, session: SessionId, by: &Actor) -> Ending {
        let mut state = self.state();
        if let Some(end) = state.tunnels.get(&session) {
            let cause = EndCause {
                reason: EndReason::Operator,
                by: Some(by.clone()),
            };
            signal(end, cause);
            return Ending::Signalled(Closing(vec![end.clone()]));
        }
        state
            .withdraw(session)
            .map_or(Ending::Neither, Ending::Withdrawn)
    }

    pub(super) fn withdraw(&self, session: SessionId) -> Option<PendingTicket> {
        self.state().withdraw(session)
    }

    pub(super) fn take_expired(&self, now: DateTime<Utc>) -> Vec<PendingTicket> {
        let mut state = self.state();
        let mut expired: Vec<PendingTicket> = state
            .tickets
            .extract_if(|_, ticket| ticket.is_expired(now))
            .map(|(_, ticket)| ticket)
            .collect();
        expired.extend(
            state
                .spent
                .extract_if(|_, ticket| ticket.is_expired(now))
                .map(|(_, ticket)| ticket),
        );
        expired
    }

    /// Restored tickets come back without a token, so they stay refused until the sweep ends them.
    /// They may push the count past `MAX_PENDING`; `issue` refuses while it is over.
    pub(super) fn restore(&self, tickets: Vec<PendingTicket>) {
        self.state()
            .spent
            .extend(tickets.into_iter().map(|ticket| (ticket.session, ticket)));
    }

    /// Works under one lock, so no ticket becomes a tunnel in between (D-10).
    /// The caller ends the withdrawn tickets' rows and bounds the wait on `Closing`.
    pub(super) fn shut_down(&self) -> (Closing, Vec<PendingTicket>) {
        let mut state = self.state();
        state.shutting_down = true;
        let ends: Vec<EndSignal> = state.tunnels.values().cloned().collect();
        let mut tickets: Vec<PendingTicket> =
            state.tickets.drain().map(|(_, ticket)| ticket).collect();
        tickets.extend(state.spent.drain().map(|(_, ticket)| ticket));
        let relays: Vec<EndSignal> = state.files_relays.values().cloned().collect();
        drop(state);
        for end in &ends {
            let cause = EndCause {
                reason: EndReason::HubShutdown,
                by: None,
            };
            signal(end, cause);
        }
        (Closing([ends, relays].concat()), tickets)
    }
}
