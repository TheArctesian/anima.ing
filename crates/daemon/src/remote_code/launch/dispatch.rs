use super::{
    DispatchGrant, GrantReference, LaunchState, LiveLease, NotCreated, Progress, Refusal,
    Reservation, Sample, Spawn, SpawnPermit, Store, StoreError, VerifiedCreation, find, lower,
    sample,
};
use agent_threads::drive::Mode;

impl Store {
    /// Execute at the holder boundary, after preparing all I/O needed to issue the OS call.
    /// `immediate` must reread policy and attest the prepared program and canonical folder.
    /// `spawn` issues the OS call without awaiting work or preparing more resources.
    ///
    /// # Errors
    /// The original tuple could not be verified or the durable state could not be saved.
    pub fn dispatch(
        &self,
        authority: (DispatchGrant, &LiveLease),
        immediate: impl Fn(&Reservation) -> Result<Mode, Refusal>,
        spawn: impl FnOnce(&Reservation, Mode, SpawnPermit) -> Spawn,
    ) -> Result<Progress, StoreError> {
        self.dispatch_with_clock(authority, (immediate, sample), spawn)
    }

    fn dispatch_with_clock(
        &self,
        (grant, lease): (DispatchGrant, &LiveLease),
        (immediate, clock): (
            impl Fn(&Reservation) -> Result<Mode, Refusal>,
            impl Fn() -> Option<Sample>,
        ),
        spawn: impl FnOnce(&Reservation, Mode, SpawnPermit) -> Spawn,
    ) -> Result<Progress, StoreError> {
        let mut locked = self.lock()?;
        let saved = find(&mut locked.journal, &grant.request)?;
        if saved.progress() != Progress::Reserved {
            return Ok(saved.progress());
        }
        let Ok(access) = immediate(saved) else {
            saved.state = LaunchState::Closed(NotCreated::Refused);
            locked.save()?;
            return Ok(Progress::Closed(NotCreated::Refused));
        };
        if !clock().is_some_and(|now| grant.live(lease, now)) {
            return Ok(Progress::WaitingForGrant);
        }
        let access = lower(saved.parameters.access, access);
        let reference = GrantReference {
            authorization: grant.grant.authorization,
            valid_until: grant.grant.valid_until,
        };
        saved.state = LaunchState::Dispatching {
            grant: reference.clone(),
            access,
        };
        let reservation = saved.clone();
        locked.save()?;
        if !immediate(&reservation).is_ok_and(|current| lower(access, current) == access)
            || !clock().is_some_and(|now| grant.live(lease, now))
        {
            return Ok(Progress::Reconciling);
        }
        let permit = SpawnPermit {
            request: grant.request,
            grant: reference,
        };
        let Spawn::Created(proof) = spawn(&reservation, access, permit) else {
            return Ok(Progress::Reconciling);
        };
        if proof.request != grant.request {
            return Err(StoreError::Conflict);
        }
        let saved = find(&mut locked.journal, &grant.request)?;
        if created(&mut saved.state, proof)? {
            locked.save()?;
        }
        Ok(Progress::Created)
    }

    /// Reconcile with proof obtained at the original OS call, retained by the live holder.
    ///
    /// # Errors
    /// The proof does not match the original marker, or storage is unavailable.
    pub fn recover_created(&self, proof: VerifiedCreation) -> Result<Progress, StoreError> {
        let mut locked = self.lock()?;
        let saved = find(&mut locked.journal, &proof.request)?;
        if created(&mut saved.state, proof)? {
            locked.save()?;
        }
        Ok(Progress::Created)
    }
}

fn created(state: &mut LaunchState, proof: VerifiedCreation) -> Result<bool, StoreError> {
    if proof.process.0.is_nil() {
        return Err(StoreError::Invalid);
    }
    match state {
        LaunchState::Dispatching { grant, access } if *grant == proof.grant => {
            *state = LaunchState::Created {
                grant: proof.grant,
                access: *access,
                process: proof.process,
                withdrawn: false,
                ended: false,
            };
            Ok(true)
        }
        LaunchState::Created { grant, process, .. }
            if *grant == proof.grant && *process == proof.process =>
        {
            Ok(false)
        }
        _ => Err(StoreError::Conflict),
    }
}

#[cfg(test)]
pub(in crate::remote_code) mod tests;
