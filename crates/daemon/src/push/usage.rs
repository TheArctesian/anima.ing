//! The agent usage waiting for the hub (AIS-019, K8 Section 4); a day waits until accepted or refused as `MALFORMED`.
//! The accounts go, as keys with their addresses, when they changed since the last accepted batch (AIS-022, THR-149).

use std::collections::BTreeMap;

use daemon_core::agent_usage::account::SALT_BYTES;
use daemon_core::agent_usage::{MAX_SESSION_DAYS, SessionDay, UsageState};
use daemon_core::wire::{BatchAccount, BatchAgentUsage};

use super::accounts;
use crate::sampling::agent_usage::{DayKey, ProgramAccount, UsageReport, day_key};

#[derive(Debug, Default)]
pub struct UsageBacklog {
    state: Option<UsageState>,
    announce: bool,
    waiting: BTreeMap<DayKey, SessionDay>,
    in_flight: Vec<DayKey>,
    accounts: Option<Vec<ProgramAccount>>,
    /// In memory only: a new start or enrolment asks for the salt again.
    salt: Option<[u8; SALT_BYTES]>,
    accepted: Option<Vec<BatchAccount>>,
    accounts_in_flight: Option<Vec<BatchAccount>>,
}

impl UsageBacklog {
    pub fn take(&mut self, report: UsageReport) {
        if self.state != Some(report.state) {
            self.state = Some(report.state);
            self.announce = true;
        }
        for day in report.session_days {
            self.waiting.insert(day_key(&day), day);
        }
        if report.accounts.is_some() {
            self.accounts = report.accounts;
        }
    }

    /// A salt that does not read is ignored, so the accounts wait for the next answer (K8 Section 8.2).
    pub fn take_salt(&mut self, text: Option<&str>) {
        if let Some(salt) = text.and_then(accounts::salt) {
            self.salt = Some(salt);
        }
    }

    fn changed_accounts(&self) -> Option<Vec<BatchAccount>> {
        let accounts = self.accounts.as_ref()?;
        let now = accounts::outgoing(self.salt.as_ref(), accounts);
        (self.accepted.as_ref() != Some(&now)).then_some(now)
    }

    pub fn outgoing(&mut self) -> Option<BatchAgentUsage> {
        let state = self.state?;
        let accounts = self.changed_accounts();
        if !self.announce && self.waiting.is_empty() && accounts.is_none() {
            return None;
        }
        let days: Vec<&SessionDay> = self.waiting.values().take(MAX_SESSION_DAYS).collect();
        self.in_flight = days.iter().map(|day| day_key(day)).collect();
        let days: Vec<SessionDay> = days.into_iter().cloned().collect();
        self.accounts_in_flight.clone_from(&accounts);
        Some(BatchAgentUsage {
            accounts,
            ..BatchAgentUsage::new(state, &days)
        })
    }

    /// The batch of [`outgoing`](Self::outgoing) was accepted or refused as `MALFORMED`; nothing was taken since.
    pub fn done(&mut self) {
        for key in self.in_flight.drain(..) {
            self.waiting.remove(&key);
        }
        if let Some(sent) = self.accounts_in_flight.take() {
            self.accepted = Some(sent);
        }
        self.announce = false;
    }
}

#[cfg(test)]
mod account_tests;

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate};
    use daemon_core::agent_usage::Source;

    use super::*;

    fn day(id: u32) -> SessionDay {
        SessionDay {
            source: Source::ClaudeCode,
            id: format!("s{id:02}"),
            day: NaiveDate::from_ymd_opt(2026, 11, 8).expect("a day"),
            started_at: DateTime::UNIX_EPOCH,
            ended_at: DateTime::UNIX_EPOCH,
            repository: None,
            prompts: 1,
            tool_calls: 0,
            models: Vec::new(),
        }
    }

    fn reading(days: Vec<SessionDay>) -> UsageReport {
        UsageReport {
            state: UsageState::Reading,
            session_days: days,
            accounts: None,
        }
    }

    #[test]
    fn ais_019_the_state_goes_once_and_session_days_16_to_a_batch() {
        let mut backlog = UsageBacklog::default();
        assert_eq!(backlog.outgoing(), None, "nothing read yet");
        backlog.take(reading((0..20).map(day).collect()));
        let first = backlog.outgoing().expect("the first batch");
        assert_eq!(
            (first.state.as_str(), first.session_days.len()),
            ("reading", 16)
        );
        assert_eq!(backlog.outgoing(), Some(first), "kept until done");
        backlog.done();
        let rest = backlog.outgoing().expect("the rest");
        assert_eq!(rest.session_days.len(), 4);
        assert_eq!(rest.session_days[0].id, "s16");
        backlog.done();
        assert_eq!(backlog.outgoing(), None);
        backlog.take(reading(Vec::new()));
        assert_eq!(backlog.outgoing(), None, "the same state is not sent again");
    }

    #[test]
    fn ais_024_a_daemon_that_cannot_read_announces_its_state_and_no_days() {
        let mut backlog = UsageBacklog::default();
        backlog.take(UsageReport {
            state: UsageState::SystemUnit,
            session_days: Vec::new(),
            accounts: None,
        });
        let sent = backlog.outgoing().expect("the state");
        assert_eq!(
            (sent.state.as_str(), sent.session_days.len()),
            ("system_unit", 0)
        );
        backlog.done();
        assert_eq!(backlog.outgoing(), None);
    }

    #[test]
    fn ais_019_a_newer_count_of_a_waiting_day_replaces_it() {
        let mut backlog = UsageBacklog::default();
        backlog.take(reading(vec![day(1)]));
        let newer = SessionDay {
            prompts: 5,
            ..day(1)
        };
        backlog.take(reading(vec![newer]));
        let sent = backlog.outgoing().expect("one day");
        assert_eq!(sent.session_days.len(), 1);
        assert_eq!(sent.session_days[0].prompts, 5);
    }
}
