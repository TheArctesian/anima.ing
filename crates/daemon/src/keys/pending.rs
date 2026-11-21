//! What the key store still owes after the Keychain made a call wait for the person (R-322):
//! pure, from the answers and the clock. A save that waited is tried again on the schedule of
//! [`Tries`]; the cache is only a cache, so it is kept in memory meanwhile. A delete that
//! waited, as after a refused token (R-147), is tried again at each round until it answers, as
//! it removes key values; a save that lands late must not leave them.

use chrono::{DateTime, Utc};

use super::store::KeyStoreError;
use crate::keychain_wait::Tries;

/// The calls of the key store that wait for the Keychain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    /// A save waited: saves are tried on this schedule only.
    saves: Option<Tries>,
    /// A delete of the whole cache waited: it is tried at each round until it answers.
    delete: bool,
}

impl Pending {
    /// Whether a call of the store waits for the person: the mark for the app is then put.
    #[must_use]
    pub const fn waits(&self) -> bool {
        self.saves.is_some() || self.delete
    }

    /// Whether a delete is owed: it is tried at each round.
    #[must_use]
    pub const fn delete_owed(&self) -> bool {
        self.delete
    }

    /// Whether a save may run at `now`: always, unless saves wait and the next try is later.
    #[must_use]
    pub fn save_due(&self, now: DateTime<Utc>) -> bool {
        self.saves.is_none_or(|tries| tries.due(now))
    }

    /// Takes the answer of a save at `now`. Returns true for the first save that waited, which
    /// is logged. A save that answers also settles a delete that is owed: the record it wrote
    /// holds no value of before.
    pub fn saved(&mut self, answer: &Result<(), KeyStoreError>, now: DateTime<Utc>) -> bool {
        match answer {
            Ok(()) => {
                *self = Self::default();
                false
            }
            Err(KeyStoreError::KeychainWaits) => {
                let first = self.saves.is_none();
                self.saves = Some(
                    self.saves
                        .map_or(Tries::first(now), |tries| tries.again(now)),
                );
                first
            }
            Err(_) => false,
        }
    }

    /// Takes the answer of a delete. Returns true for the first delete that waited, which is
    /// logged. Any other failure leaves an owed delete owed.
    pub fn deleted(&mut self, answer: &Result<(), KeyStoreError>) -> bool {
        match answer {
            Ok(()) => {
                *self = Self::default();
                false
            }
            Err(KeyStoreError::KeychainWaits) => {
                let first = !self.delete;
                self.delete = true;
                first
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;
    use crate::config::home::HomeError;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + TimeDelta::seconds(seconds)
    }

    fn other() -> Result<(), KeyStoreError> {
        Err(KeyStoreError::Folder(HomeError::NoFolder))
    }

    #[test]
    fn r_322_a_save_that_waits_is_logged_once_and_tried_again_every_30_s_until_it_answers() {
        let mut pending = Pending::default();
        assert!(pending.save_due(at(0)) && !pending.waits());
        assert!(
            pending.saved(&Err(KeyStoreError::KeychainWaits), at(0)),
            "logged"
        );
        assert!(pending.waits(), "the mark is put");
        assert!(!pending.save_due(at(29)), "skipped until the next try");
        assert!(pending.save_due(at(30)));
        assert!(
            !pending.saved(&Err(KeyStoreError::KeychainWaits), at(30)),
            "not logged again"
        );
        assert!(!pending.save_due(at(59)));
        assert!(!pending.saved(&Ok(()), at(60)));
        assert_eq!(
            pending,
            Pending::default(),
            "saves resume and the mark goes"
        );
    }

    #[test]
    fn r_147_a_delete_that_waits_is_owed_until_a_delete_or_a_save_answers() {
        let mut pending = Pending::default();
        assert!(
            pending.deleted(&Err(KeyStoreError::KeychainWaits)),
            "logged"
        );
        assert!(pending.delete_owed() && pending.waits());
        assert!(
            !pending.deleted(&Err(KeyStoreError::KeychainWaits)),
            "not logged again"
        );
        assert!(!pending.deleted(&other()));
        assert!(pending.delete_owed(), "another failure leaves it owed");
        assert!(!pending.saved(&other(), at(0)));
        assert!(pending.delete_owed());
        pending.deleted(&Ok(()));
        assert_eq!(pending, Pending::default());

        pending.deleted(&Err(KeyStoreError::KeychainWaits));
        pending.saved(&Ok(()), at(0));
        assert!(
            !pending.delete_owed(),
            "a save that answered left no value of before"
        );
    }
}
