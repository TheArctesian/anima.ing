use std::{path::Path, time::Duration};

use agent_threads::drive::Mode;
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};
use tokio::process::{Child, Command};
use uuid::Uuid;

use super::{
    DispatchGrant, GrantReference, LaunchState, LiveLease, NotCreated, ProcessIdentity, Progress,
    Refusal, Reservation, Sample, Store, StoreError, find, lower, sample,
};
use crate::remote_code::wire::DispatchRequest;

pub enum Dispatch {
    NoChild(Progress),
    Spawned(Box<Spawned>),
}

/// Creation evidence stays with the actual child returned by the guarded OS call.
pub struct Spawned {
    child: Child,
    request: DispatchRequest,
    grant: GrantReference,
    process: ProcessIdentity,
    recorded: bool,
}

impl Store {
    /// The holder supplies a fully prepared command from the selected provider launcher.
    /// Final policy validation and the elapsed check finish before the concrete OS call.
    ///
    /// # Errors
    /// The reservation could not be verified or the dispatch marker could not be stored.
    pub fn spawn(
        &self,
        authority: (DispatchGrant, &LiveLease),
        immediate: impl Fn(&Reservation) -> Result<Mode, Refusal>,
        prepared: (Mode, Command),
    ) -> Result<Dispatch, StoreError> {
        self.spawn_with_clock(authority, (immediate, sample), prepared)
    }

    fn spawn_with_clock(
        &self,
        (grant, lease): (DispatchGrant, &LiveLease),
        (immediate, clock): (
            impl Fn(&Reservation) -> Result<Mode, Refusal>,
            impl Fn() -> Option<Sample>,
        ),
        (access, mut command): (Mode, Command),
    ) -> Result<Dispatch, StoreError> {
        let mut locked = self.lock()?;
        let saved = find(&mut locked.journal, &grant.request)?;
        if saved.progress() != Progress::Reserved {
            return Ok(Dispatch::NoChild(saved.progress()));
        }
        let bounded = |reservation: &Reservation| {
            immediate(reservation)
                .is_ok_and(|mode| lower(reservation.parameters.access, mode) == access)
        };
        if !prepared_for(saved, &command) || !bounded(saved) {
            saved.state = LaunchState::Closed(NotCreated::Refused);
            locked.save()?;
            return Ok(Dispatch::NoChild(Progress::Closed(NotCreated::Refused)));
        }
        if !clock().is_some_and(|now| grant.live(lease, now)) {
            return Ok(Dispatch::NoChild(Progress::WaitingForGrant));
        }
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
        command.process_group(0);
        if !bounded(&reservation) || !clock().is_some_and(|now| grant.live(lease, now)) {
            return Ok(Dispatch::NoChild(Progress::Reconciling));
        }
        let Ok(child) = command.spawn() else {
            return Ok(Dispatch::NoChild(Progress::Reconciling));
        };
        Ok(Dispatch::Spawned(Box::new(Spawned {
            child,
            request: grant.request,
            grant: reference,
            process: ProcessIdentity(Uuid::now_v7()),
            recorded: false,
        })))
    }
}

fn prepared_for(saved: &Reservation, command: &Command) -> bool {
    let command = command.as_std();
    Path::new(command.get_program()).is_absolute()
        && command.get_current_dir() == Some(saved.parameters.folder.as_path())
}

impl Spawned {
    pub(crate) fn exited(&self) -> std::io::Result<bool> {
        let Some(pid) = child_pid(&self.child) else {
            return Ok(true);
        };
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .map(|status| status.is_some())
        .map_err(Into::into)
    }

    pub(crate) async fn end(&mut self) -> std::io::Result<()> {
        let Some(group) = child_pid(&self.child) else {
            return Ok(());
        };
        // The pending child has no other waiter. Its PID cannot be reused as long as
        // this holder leaves it waitable, even when the group leader has already exited.
        match kill_process_group(group, Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(error.into()),
        }
        if !self.exited()? {
            self.child.start_kill()?;
        }
        tokio::time::timeout(Duration::from_secs(2), self.child.wait())
            .await?
            .map(|_| ())
    }

    /// Record creation using the retained live child. The holder retains this value if
    /// storage fails and can retry against the same marker.
    ///
    /// # Errors
    /// Child status could not be verified, the original marker differs, or storage failed.
    pub fn record(&mut self, store: &Store) -> Result<Progress, StoreError> {
        let mut locked = store.lock()?;
        if self.exited()? {
            return Ok(Progress::Reconciling);
        }
        if self.recorded {
            return Ok(Progress::Created);
        }
        let saved = find(&mut locked.journal, &self.request)?;
        match &saved.state {
            LaunchState::Created { grant, process, .. }
                if *grant == self.grant && *process == self.process => {}
            LaunchState::Dispatching { grant, access } if *grant == self.grant => {
                saved.state = LaunchState::Created {
                    grant: self.grant.clone(),
                    access: *access,
                    process: self.process.clone(),
                    withdrawn: false,
                    ended: false,
                };
            }
            _ => return Err(StoreError::Conflict),
        }
        locked.save()?;
        self.recorded = true;
        Ok(Progress::Created)
    }

    /// Release the child to the holder's normal attach and waiter paths after recording it.
    ///
    /// # Errors
    /// The original child is returned still joined to its pending creation evidence.
    pub fn into_child(self) -> Result<(Child, ProcessIdentity), Box<Self>> {
        if !self.recorded {
            return Err(Box::new(self));
        }
        Ok((self.child, self.process))
    }
}

fn child_pid(child: &Child) -> Option<Pid> {
    child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .and_then(Pid::from_raw)
}

#[cfg(test)]
pub(in crate::remote_code) mod tests;
