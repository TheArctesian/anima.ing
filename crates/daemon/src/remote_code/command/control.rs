//! Persist the sequence of offers across process replacement and daemon restart.

use super::{Offer, SavedOffer, Target, receipts::digest};
use crate::remote_code::{
    Store, StoreError,
    launch::{LaunchState, ProcessIdentity, find},
    wire::DispatchRequest,
};

impl Store {
    /// Give an observed offer its persistent revision, preserving it on an exact refresh.
    ///
    /// # Errors
    /// The live process does not match the launch or the offer cannot be stored or published.
    pub fn revise_control(
        &self,
        request: &DispatchRequest,
        live: &ProcessIdentity,
        offer: &mut Offer,
    ) -> Result<(), StoreError> {
        if !offer.observes(live) {
            return Err(StoreError::Conflict);
        }
        let mut locked = self.lock()?;
        let key = locked.journal.key;
        let launch = find(&mut locked.journal, request)?;
        if !matches!(&launch.state, LaunchState::Created { process, withdrawn: false, ended: false, .. } if process == live)
        {
            return Err(StoreError::Conflict);
        }
        let hash = offer.hash(&key)?;
        let current = launch.control.as_ref();
        if let Some(current) = current.filter(|current| current.hash == hash) {
            offer.revision = current.revision;
            return Ok(());
        }
        let revision = current.map_or(1, |current| current.revision + 1);
        if !(1..(1 << 53)).contains(&revision) {
            return Err(StoreError::Full);
        }
        offer.revision = revision;
        launch.control = Some(SavedOffer { revision, hash });
        locked.save()
    }

    pub(in crate::remote_code) fn confirms_offer(
        &self,
        request: &DispatchRequest,
        offer: &Offer,
    ) -> Result<bool, StoreError> {
        let mut locked = self.lock()?;
        let key = locked.journal.key;
        let saved = find(&mut locked.journal, request)?;
        Ok(matches!(&saved.state,
            LaunchState::Created { process, withdrawn: false, ended: false, .. } if offer.observes(process)
        ) && saved
            .control
            .as_ref()
            .is_some_and(|current| current.matches(offer, &key)))
    }
}

impl SavedOffer {
    pub(super) fn matches(&self, offer: &Offer, key: &[u8; 32]) -> bool {
        self.revision == offer.revision && offer.hash(key).is_ok_and(|hash| hash == self.hash)
    }
}

impl Offer {
    pub(super) fn observes(&self, live: &ProcessIdentity) -> bool {
        self.process.parse::<uuid::Uuid>() == Ok(live.id())
            && self.stop_targets.iter().all(|target| {
                let process = match target {
                    Target::End { process } | Target::Interrupt { process, .. } => process,
                };
                process == &self.process
            })
    }

    fn hash(&self, key: &[u8; 32]) -> Result<[u8; 32], StoreError> {
        digest(
            &(
                &self.process,
                &self.actions,
                &self.stop_targets,
                &self.requests,
            ),
            key,
        )
    }
}
