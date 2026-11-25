use super::*;

impl Store {
    /// Cancel a delivered, undispatched prompt after a validated stop won its target.
    ///
    /// # Errors
    /// An earlier command conflicts, or the cancellation could not be stored.
    pub fn cancel_prompt(
        &self,
        stop: &Delivered,
        prompt: &Delivered,
    ) -> Result<Consumption, StoreError> {
        let mut locked = self.lock()?;
        if stop.owner != prompt.owner
            || stop.launch != prompt.launch
            || stop.thread != prompt.thread
            || !matches!(stop.payload, Payload::Stop { .. })
            || !matches!(prompt.payload, Payload::Prompt { .. })
        {
            return Err(StoreError::Conflict);
        }
        let stopped = locked
            .journal
            .commands
            .get(&stop.id)
            .ok_or(StoreError::Conflict)?;
        if stopped.fingerprint != fingerprint(stop, &locked.journal.key)?
            || matches!(
                stopped.state,
                CommandState::Settled(ReceiptOutcome::Refused { .. })
            )
        {
            return Err(StoreError::Conflict);
        }
        let fingerprint = fingerprint(prompt, &locked.journal.key)?;
        if let Some(prior) = locked.journal.commands.get(&prompt.id) {
            return prior.replay(prompt, fingerprint);
        }
        locked.journal.admit()?;
        let mut command = SavedCommand::new(prompt, fingerprint, &locked.journal.key)?;
        command.state = CommandState::Settled(ReceiptOutcome::Refused {
            cause: Cause::CancelledByStop,
        });
        let outcome = command.outcome(prompt);
        locked.journal.commands.insert(prompt.id.clone(), command);
        locked.save_admission()?;
        Ok(outcome)
    }
}
