use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::Sha256;
use zeroize::Zeroizing;

use super::{
    Action, Admission, Admitted, Cause, CommandState, Consumption, Delivered, Handoff, LiveControl,
    Payload, Receipt, ReceiptOutcome, RecordedBinding, SavedCommand, Target,
};
use crate::remote_code::{Store, StoreError, journal::Journal, launch::LaunchState, policy::lower};

const ANSWER_LIMIT: usize = 64 * 1024;

impl Store {
    /// The holder must serialize this call with the actual live request and local stop paths.
    /// Run the bounded storage I/O on the holder's serialized execution lane. The handoff must
    /// return `Uncertain` if it cannot prove whether the adapter accepted the action.
    ///
    /// # Errors
    /// Storage failed, or a delivery conflicts with an earlier command ID.
    pub fn consume(
        &self,
        delivered: &Delivered,
        live: &LiveControl<'_>,
        handoff: impl FnOnce(&Payload) -> Handoff,
    ) -> Result<Consumption, StoreError> {
        match self.admit(delivered, Some(live))? {
            Admission::Existing(result) => Ok(result),
            Admission::Effect(ticket) => match handoff(&delivered.payload) {
                Handoff::Settled(outcome) => self.finish(ticket, outcome).map(Consumption::Settled),
                Handoff::Uncertain => Ok(Consumption::Reconciling),
            },
        }
    }

    /// Store the handoff before the serialized runner calls the holder.
    ///
    /// # Errors
    /// Storage failed or the delivered ID conflicts.
    pub fn admit(
        &self,
        delivered: &Delivered,
        live: Option<&LiveControl<'_>>,
    ) -> Result<Admission, StoreError> {
        let mut locked = self.lock()?;
        let fingerprint = fingerprint(delivered, &locked.journal.key)?;
        if let Some(previous) = locked.journal.commands.get(&delivered.id) {
            return previous
                .replay(delivered, fingerprint)
                .map(Admission::Existing);
        }
        let mut command = SavedCommand::new(delivered, fingerprint, &locked.journal.key)?;
        match live.map_or(Err(Cause::ThreadUnknown), |live| {
            checked(&locked.journal, delivered, live)
        }) {
            Err(cause) => command.state = CommandState::Settled(ReceiptOutcome::Refused { cause }),
            Ok(()) => match prior_handoff(&locked.journal, delivered, command.request.as_ref()) {
                Some(Consumption::Reconciling) => {
                    return Ok(Admission::Existing(Consumption::Reconciling));
                }
                Some(Consumption::Settled(receipt)) => {
                    command.state = CommandState::Settled(receipt.outcome);
                }
                None => {}
            },
        }
        let outcome = command.outcome(delivered);
        locked.journal.admit()?;
        locked
            .journal
            .commands
            .insert(delivered.id.clone(), command);
        locked.save_admission()?;
        if matches!(outcome, Consumption::Settled(_)) {
            return Ok(Admission::Existing(outcome));
        }
        Ok(Admission::Effect(Admitted {
            id: delivered.id.clone(),
            fingerprint,
            end: matches!(
                delivered.payload,
                Payload::Stop {
                    target: Target::End { .. }
                }
            ),
        }))
    }

    /// Record an observed provider acknowledgement against its reserved handoff.
    ///
    /// # Errors
    /// The receipt could not be persisted.
    pub fn finish(&self, ticket: Admitted, outcome: ReceiptOutcome) -> Result<Receipt, StoreError> {
        let mut locked = self.lock()?;
        let Some(saved) = locked
            .journal
            .commands
            .get_mut(&ticket.id)
            .filter(|saved| saved.fingerprint == ticket.fingerprint)
        else {
            return Err(StoreError::Conflict);
        };
        let outcome = match saved.state {
            CommandState::Settled(old) => old,
            CommandState::HandingOff { .. } => outcome,
        };
        saved.state = CommandState::Settled(outcome);
        let launch = saved.binding.launch;
        if ticket.end
            && matches!(outcome, ReceiptOutcome::Recorded { .. })
            && let Some(launch) = locked.journal.launches.get_mut(&launch)
            && let LaunchState::Created { ended, .. } = &mut launch.state
        {
            *ended = true;
        }
        locked.save()?;
        Ok(Receipt {
            id: ticket.id,
            outcome,
        })
    }
}

impl SavedCommand {
    fn new(
        delivered: &Delivered,
        fingerprint: [u8; 32],
        key: &[u8; 32],
    ) -> Result<Self, StoreError> {
        let stop = matches!(delivered.payload, Payload::Stop { .. });
        let request = match &delivered.payload {
            Payload::Answer {
                process, request, ..
            } => Some(digest(&(process, request), key)?),
            _ => None,
        };
        Ok(Self {
            binding: RecordedBinding {
                launch: delivered.launch,
                thread: delivered.thread,
            },
            fingerprint,
            request,
            state: CommandState::HandingOff { stop },
        })
    }

    fn replay(
        &self,
        delivered: &Delivered,
        fingerprint: [u8; 32],
    ) -> Result<Consumption, StoreError> {
        if self.fingerprint != fingerprint {
            return Err(StoreError::Conflict);
        }
        Ok(self.outcome(delivered))
    }

    fn outcome(&self, delivered: &Delivered) -> Consumption {
        match self.state {
            CommandState::Settled(outcome) => Consumption::Settled(Receipt {
                id: delivered.id.clone(),
                outcome,
            }),
            CommandState::HandingOff { .. } => Consumption::Reconciling,
        }
    }
}

fn fingerprint(delivered: &Delivered, key: &[u8; 32]) -> Result<[u8; 32], StoreError> {
    digest(
        &(
            &delivered.owner,
            delivered.launch,
            delivered.thread,
            &delivered.payload,
        ),
        key,
    )
}

pub(super) fn digest(
    value: &impl serde::Serialize,
    key: &[u8; 32],
) -> Result<[u8; 32], StoreError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| StoreError::Invalid)?;
    let bytes = serde_json::to_vec(value).map_err(|_| StoreError::Invalid)?;
    mac.update(&Zeroizing::new(bytes));
    Ok(mac.finalize().into_bytes().into())
}

fn checked(journal: &Journal, delivered: &Delivered, live: &LiveControl<'_>) -> Result<(), Cause> {
    let access = current_access(journal, delivered, live)?;
    let bounded = lower(access, live.ceiling);
    let mode_allowed = lower(bounded, live.mode) == live.mode;
    match &delivered.payload {
        Payload::Prompt { .. } => prompt(live, mode_allowed),
        Payload::Answer {
            process,
            revision,
            request,
            answer,
        } => {
            if !answer.declines() && !mode_allowed {
                return Err(Cause::AccessChanged);
            }
            if serde_json::to_vec(answer)
                .map_or(true, |bytes| Zeroizing::new(bytes).len() > ANSWER_LIMIT)
            {
                return Err(Cause::InvalidAnswer);
            }
            live.offer.answer((process, *revision, request), answer)
        }
        Payload::Stop { target } => {
            if live.offer.actions.contains(&Action::Stop)
                && live.offer.stop_targets.contains(target)
            {
                Ok(())
            } else {
                Err(Cause::TargetStale)
            }
        }
    }
}

fn current_access(
    journal: &Journal,
    delivered: &Delivered,
    live: &LiveControl<'_>,
) -> Result<agent_threads::drive::Mode, Cause> {
    let launch = journal
        .launches
        .get(&delivered.launch)
        .filter(|saved| saved.owns(&delivered.owner, delivered.thread))
        .ok_or(Cause::ThreadUnknown)?;
    let LaunchState::Created {
        process,
        access,
        withdrawn,
        ended,
        ..
    } = &launch.state
    else {
        return Err(Cause::ThreadUnknown);
    };
    if *ended {
        return Err(Cause::ThreadEnded);
    }
    if *withdrawn || !live.publishable {
        return Err(Cause::FolderNotPermitted);
    }
    if process != live.process || !live.offer.observes(process) {
        return Err(Cause::TargetStale);
    }
    if launch
        .control
        .as_ref()
        .is_none_or(|control| !control.matches(live.offer, &journal.key))
    {
        return Err(Cause::TargetStale);
    }
    Ok(*access)
}

fn prompt(live: &LiveControl<'_>, mode_allowed: bool) -> Result<(), Cause> {
    if !live.threads_on {
        return Err(Cause::ThreadsOff);
    }
    if !live.folder_permitted {
        return Err(Cause::FolderNotPermitted);
    }
    if !mode_allowed {
        return Err(Cause::AccessChanged);
    }
    if !live.offer.actions.contains(&Action::Prompt) {
        return Err(Cause::Busy);
    }
    Ok(())
}

fn prior_handoff(
    journal: &Journal,
    delivered: &Delivered,
    request: Option<&[u8; 32]>,
) -> Option<Consumption> {
    journal
        .commands
        .values()
        .filter(|command| {
            command.binding.launch == delivered.launch && command.binding.thread == delivered.thread
        })
        .find_map(
            |command| match (&delivered.payload, &command.request, &command.state) {
                (Payload::Prompt { .. }, _, CommandState::HandingOff { stop: true }) => {
                    Some(Consumption::Reconciling)
                }
                (Payload::Answer { .. }, Some(earlier), state) if Some(earlier) == request => {
                    match state {
                        CommandState::HandingOff { .. } => Some(Consumption::Reconciling),
                        CommandState::Settled(ReceiptOutcome::Recorded { .. }) => {
                            Some(Consumption::Settled(Receipt {
                                id: delivered.id.clone(),
                                outcome: ReceiptOutcome::Refused {
                                    cause: Cause::RequestResolved,
                                },
                            }))
                        }
                        CommandState::Settled(ReceiptOutcome::Refused { .. }) => None,
                    }
                }
                _ => None,
            },
        )
}

mod cancel;

#[cfg(test)]
mod tests;
