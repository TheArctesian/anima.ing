//! A renewed token whose save waits for the Keychain (R-322, K8 Section 5); it is used only once stored.
//! No other renewal runs meanwhile, and a token whose save waited is kept: a late save may still store it.

use chrono::{DateTime, Utc};

use crate::keychain_wait::Tries;
use crate::token_store::{Credential, TokenStoreError};

pub struct Unstored {
    credential: Credential,
    tries: Tries,
}

pub enum Stored {
    Yes(Credential),
    Later(Unstored),
    /// `first` marks the first wait of this token's save, which is logged.
    Waits {
        unstored: Unstored,
        first: bool,
    },
    /// Another failure after a wait, which may have stored the token: kept and tried again.
    Unsure(Unstored, TokenStoreError),
    /// Another failure of the first save: nothing can be stored, and the token is dropped.
    No(TokenStoreError),
}

pub fn store(
    next: Credential,
    now: DateTime<Utc>,
    save: impl FnOnce(&Credential) -> Result<(), TokenStoreError>,
) -> Stored {
    match save(&next) {
        Ok(()) => Stored::Yes(next),
        Err(TokenStoreError::KeychainWaits) => Stored::Waits {
            unstored: Unstored {
                credential: next,
                tries: Tries::first(now),
            },
            first: true,
        },
        Err(error) => Stored::No(error),
    }
}

impl Unstored {
    /// After a failure other than a wait, an item that holds this token was stored late by a waited save.
    pub fn retry(
        mut self,
        now: DateTime<Utc>,
        save: impl FnOnce(&Credential) -> Result<(), TokenStoreError>,
        read_back: impl FnOnce() -> Result<Option<Credential>, TokenStoreError>,
    ) -> Stored {
        if !self.tries.due(now) {
            return Stored::Later(self);
        }
        let error = match save(&self.credential) {
            Ok(()) => return Stored::Yes(self.credential),
            Err(error) => error,
        };
        self.tries = self.tries.again(now);
        if matches!(error, TokenStoreError::KeychainWaits) {
            return Stored::Waits {
                unstored: self,
                first: false,
            };
        }
        let mine = self.credential.token.hash();
        if matches!(read_back(), Ok(Some(stored)) if stored.token.hash() == mine) {
            return Stored::Yes(self.credential);
        }
        Stored::Unsure(self, error)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use chrono::TimeDelta;
    use daemon_core::wire::DaemonToken;

    use super::*;
    use crate::config::origin::Origin;

    type Answer = Result<(), TokenStoreError>;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + TimeDelta::seconds(seconds)
    }

    fn credential(seed: u8) -> Credential {
        Credential {
            token: DaemonToken::from_random([seed; 32]),
            expires_at: at(0) + TimeDelta::days(365),
            hub: Origin::parse("https://hub.example").expect("origin"),
        }
    }

    fn waits() -> Answer {
        Err(TokenStoreError::KeychainWaits)
    }

    fn untouched() -> Result<Option<Credential>, TokenStoreError> {
        panic!("not read back")
    }

    fn waiting(stored: Stored, first_wait: bool) -> Unstored {
        match stored {
            Stored::Waits { unstored, first } => {
                assert_eq!(
                    first, first_wait,
                    "the first wait is the one that is logged"
                );
                unstored
            }
            _ => panic!("not a wait"),
        }
    }

    fn is_next(stored: &Stored) -> bool {
        matches!(stored, Stored::Yes(stored) if stored.token.hash() == credential(2).token.hash())
    }

    #[test]
    fn met_013_a_renewed_token_whose_save_waits_is_kept_unused_and_stored_later() {
        let saves = RefCell::new(0);
        let save = |answer: Answer| {
            let saves = &saves;
            move |next: &Credential| {
                assert!(next.token.hash() == credential(2).token.hash());
                *saves.borrow_mut() += 1;
                answer
            }
        };
        let unstored = waiting(store(credential(2), at(0), save(waits())), true);
        let Stored::Later(unstored) = unstored.retry(at(29), save(Ok(())), untouched) else {
            panic!("tried before 30 s");
        };
        assert_eq!(*saves.borrow(), 1, "a retry that is not due tries nothing");
        let unstored = waiting(unstored.retry(at(30), save(waits()), untouched), false);
        assert!(is_next(&unstored.retry(at(60), save(Ok(())), untouched)));
        assert_eq!(*saves.borrow(), 3);
    }

    #[test]
    fn met_013_saves_that_wait_are_tried_every_30_s_for_10_minutes_then_every_5_minutes() {
        let mut unstored = waiting(store(credential(2), at(0), |_| waits()), true);
        let mut now = at(0);
        for _ in 0..20 {
            now += TimeDelta::seconds(30);
            unstored = waiting(unstored.retry(now, |_| waits(), untouched), false);
        }
        let later = unstored.retry(
            now + TimeDelta::seconds(299),
            |_| panic!("too soon"),
            untouched,
        );
        let Stored::Later(unstored) = later else {
            panic!("tried too soon");
        };
        let stored = unstored.retry(now + TimeDelta::minutes(5), |_| Ok(()), untouched);
        assert!(is_next(&stored));
    }

    #[test]
    fn met_013_a_first_save_that_fails_otherwise_drops_the_new_token() {
        let failed = store(credential(2), at(0), |_| Err(TokenStoreError::Corrupt));
        assert!(matches!(failed, Stored::No(TokenStoreError::Corrupt)));
    }

    #[test]
    fn met_013_after_a_wait_another_failure_uses_the_token_that_a_late_save_stored() {
        let unstored = waiting(store(credential(2), at(0), |_| waits()), true);
        let late = || Ok(Some(credential(2)));
        let stored = unstored.retry(at(30), |_| Err(TokenStoreError::Corrupt), late);
        assert!(is_next(&stored), "the Keychain holds it: used, not dropped");
    }

    #[test]
    fn met_013_after_a_wait_another_failure_keeps_the_token_when_the_item_holds_another() {
        let unstored = waiting(store(credential(2), at(0), |_| waits()), true);
        let old = || Ok(Some(credential(1)));
        let Stored::Unsure(unstored, TokenStoreError::Corrupt) =
            unstored.retry(at(30), |_| Err(TokenStoreError::Corrupt), old)
        else {
            panic!("dropped");
        };
        let unread = || Err(TokenStoreError::KeychainWaits);
        let Stored::Unsure(unstored, _) =
            unstored.retry(at(60), |_| Err(TokenStoreError::Corrupt), unread)
        else {
            panic!("dropped when the read back failed");
        };
        assert!(is_next(&unstored.retry(at(90), |_| Ok(()), untouched)));
    }
}
