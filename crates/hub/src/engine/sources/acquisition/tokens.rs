//! Installation tokens, held in hub memory only (PRJ-018, K-E1 Section 7).
//!
//! A token is never written to the database, a log, an audit entry or an error, and has no
//! `Debug`. The map's lock is never held across an `await`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use chrono::{DateTime, TimeDelta, Utc};
use source_exchange::Minted;

use crate::{fleet::credentials::Secret, ids::SourceAccountId};

const REMINT_BEFORE: TimeDelta = TimeDelta::minutes(5);

struct Held {
    token: Arc<Secret>,
    expires_at: DateTime<Utc>,
}

#[derive(Default)]
struct Map {
    held: HashMap<SourceAccountId, Held>,
    forgets: u64,
}

/// Taken before a mint starts: a keep with a ticket older than the last forget stores nothing,
/// so a token minted while its account was removed is not left in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);

#[derive(Clone, Default)]
pub struct Tokens(Arc<Mutex<Map>>);

impl Tokens {
    fn with<T>(&self, f: impl FnOnce(&mut Map) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    #[must_use]
    pub fn fresh(&self, account: SourceAccountId, now: DateTime<Utc>) -> Option<Arc<Secret>> {
        self.with(|map| {
            map.held
                .get(&account)
                .filter(|h| h.expires_at - now > REMINT_BEFORE)
                .map(|h| h.token.clone())
        })
    }

    #[must_use]
    pub fn ticket(&self) -> Ticket {
        self.with(|map| Ticket(map.forgets))
    }

    /// The check and the insert hold the one lock that `forget` takes, so a `forget` either drops
    /// the token after this keep or makes this keep store nothing.
    #[must_use]
    pub fn keep(&self, account: SourceAccountId, minted: Minted, ticket: Ticket) -> Arc<Secret> {
        let token = Arc::new(Secret::from(minted.token));
        let kept = Held {
            token: token.clone(),
            expires_at: minted.expires_at,
        };
        self.with(|map| {
            if map.forgets == ticket.0 {
                map.held.insert(account, kept);
            }
        });
        token
    }

    pub fn forget(&self, account: SourceAccountId) {
        self.with(|map| {
            map.forgets = map.forgets.wrapping_add(1);
            map.held.remove(&account)
        });
    }
}
