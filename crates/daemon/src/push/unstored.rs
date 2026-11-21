//! A renewed token whose save waits for the person in the Keychain (R-322, K8 Section 5).
//!
//! The daemon uses a new token only once it is stored. While its save waits, the daemon keeps
//! the new token in memory, unused, pushes with the current one (the hub accepts it until the
//! new one is first used) and tries the save again on the schedule of
//! [`keychain_wait::next_try`]. It asks the hub for no other token meanwhile: that renewal
//! would replace the new token at the hub, and a save that macOS ends late could still put
//! the replaced one in the Keychain, where the next start would read a token the hub refuses.

use chrono::{DateTime, TimeDelta, Utc};

use crate::keychain_wait;
use crate::token_store::{Credential, TokenStoreError};

/// A renewed token that is not stored yet, and when its save is tried again.
pub struct Unstored {
    credential: Credential,
    /// How many saves of it waited.
    waited: u32,
    next_at: DateTime<Utc>,
}

/// What a try to store a renewed token ended in.
pub enum Stored {
    /// Stored: the daemon uses it from now.
    Yes(Credential),
    /// The Keychain waits for the person: kept in memory, unused, and tried again.
    Waits(Unstored),
    /// Any other failure: the new token is dropped and the current one stays.
    No(TokenStoreError),
}

/// Tries to store `next`, a token the hub just gave, with `save` at `now`.
pub fn store(
    next: Credential,
    now: DateTime<Utc>,
    save: impl FnOnce(&Credential) -> Result<(), TokenStoreError>,
) -> Stored {
    Unstored {
        credential: next,
        waited: 0,
        next_at: now,
    }
    .retry(now, save)
}

impl Unstored {
    /// Whether this is the first save that waited: the one that is logged.
    #[must_use]
    pub const fn first_wait(&self) -> bool {
        self.waited == 1
    }

    /// Tries the save again with `save` when it is due at `now`; else waits on, untried.
    pub fn retry(
        mut self,
        now: DateTime<Utc>,
        save: impl FnOnce(&Credential) -> Result<(), TokenStoreError>,
    ) -> Stored {
        if now < self.next_at {
            return Stored::Waits(self);
        }
        match save(&self.credential) {
            Ok(()) => Stored::Yes(self.credential),
            Err(TokenStoreError::KeychainWaits) => {
                let pause = keychain_wait::next_try(self.waited);
                self.waited = self.waited.saturating_add(1);
                self.next_at = now + TimeDelta::from_std(pause).unwrap_or(TimeDelta::minutes(5));
                Stored::Waits(self)
            }
            Err(error) => Stored::No(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use daemon_core::wire::DaemonToken;

    use super::*;
    use crate::config::origin::Origin;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + TimeDelta::seconds(seconds)
    }

    fn next() -> Credential {
        Credential {
            token: DaemonToken::from_random([2; 32]),
            expires_at: at(0) + TimeDelta::days(365),
            hub: Origin::parse("https://hub.example").expect("origin"),
        }
    }

    fn waits(stored: Stored) -> Unstored {
        match stored {
            Stored::Waits(unstored) => unstored,
            Stored::Yes(_) => panic!("stored"),
            Stored::No(error) => panic!("dropped: {error}"),
        }
    }

    #[test]
    fn met_013_a_renewed_token_whose_save_waits_is_kept_unused_and_stored_later() {
        let saves = RefCell::new(Vec::new());
        let log = &saves;
        let save = |answer: Result<(), TokenStoreError>| {
            move |credential: &Credential| {
                log.borrow_mut().push(credential.token.hash());
                answer
            }
        };
        let unstored = waits(store(
            next(),
            at(0),
            save(Err(TokenStoreError::KeychainWaits)),
        ));
        assert!(unstored.first_wait(), "logged once");

        let unstored = waits(unstored.retry(at(29), save(Ok(()))));
        assert_eq!(saves.borrow().len(), 1, "not tried again before 30 s");

        let unstored = waits(unstored.retry(at(30), save(Err(TokenStoreError::KeychainWaits))));
        assert!(!unstored.first_wait(), "logged only at the first wait");
        assert_eq!(saves.borrow().len(), 2);

        let Stored::Yes(stored) = unstored.retry(at(60), save(Ok(()))) else {
            panic!("the save answered");
        };
        assert!(
            stored.token.hash() == next().token.hash(),
            "the token is not lost"
        );
        assert_eq!(saves.borrow().len(), 3);
        assert!(
            saves
                .borrow()
                .iter()
                .all(|hash| *hash == next().token.hash())
        );
    }

    #[test]
    fn met_013_saves_that_wait_are_tried_every_30_s_for_10_minutes_then_every_5_minutes() {
        let mut unstored = waits(store(next(), at(0), |_| {
            Err(TokenStoreError::KeychainWaits)
        }));
        let mut now = at(0);
        for _ in 0..19 {
            now += TimeDelta::seconds(30);
            unstored = waits(unstored.retry(now, |_| Err(TokenStoreError::KeychainWaits)));
        }
        now += TimeDelta::seconds(30);
        unstored = waits(unstored.retry(now, |_| Err(TokenStoreError::KeychainWaits)));
        let untried = |_: &Credential| -> Result<(), TokenStoreError> { panic!("tried too soon") };
        unstored = waits(unstored.retry(now + TimeDelta::seconds(299), untried));
        assert!(matches!(
            unstored.retry(now + TimeDelta::minutes(5), |_| Ok(())),
            Stored::Yes(_)
        ));
    }

    #[test]
    fn met_013_a_save_that_fails_otherwise_drops_the_new_token() {
        let failed = store(next(), at(0), |_| Err(TokenStoreError::Corrupt));
        assert!(matches!(failed, Stored::No(TokenStoreError::Corrupt)));
    }
}
