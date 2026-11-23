//! The agent usage a batch announces (AIS-019, K8 Section 4).

use std::collections::HashMap;

use agent_threads::drive::AccountFacts;
use chrono::NaiveDate;
use daemon_core::agent_usage::{SessionDay, Source, UsageState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageReport {
    pub state: UsageState,
    /// The session days whose counts changed; none unless the state is `Reading`.
    pub session_days: Vec<SessionDay>,
    /// Each program's account from this computer's thread runner (THR-132); `None` keeps those sent before.
    pub accounts: Option<Vec<ProgramAccount>>,
}

/// Its identifier stays at the machine; the key made from it leaves (AIS-022, THR-149).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramAccount {
    pub source: Source,
    pub facts: AccountFacts,
}

/// One session day: the key the hub upserts by (K8 Section 4).
pub type DayKey = (Source, String, NaiveDate);

#[must_use]
pub fn day_key(day: &SessionDay) -> DayKey {
    (day.source, day.id.clone(), day.day)
}

#[derive(Debug, Default)]
pub struct UsageWatch {
    handed: HashMap<DayKey, SessionDay>,
}

impl UsageWatch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn changed(&mut self, read: Vec<SessionDay>) -> Vec<SessionDay> {
        read.into_iter()
            .filter(|day| {
                let key = day_key(day);
                if self.handed.get(&key) == Some(day) {
                    return false;
                }
                self.handed.insert(key, day.clone());
                true
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::*;

    fn day(prompts: u64) -> SessionDay {
        SessionDay {
            source: Source::Codex,
            id: "t".to_owned(),
            day: NaiveDate::from_ymd_opt(2026, 11, 8).expect("a day"),
            started_at: DateTime::UNIX_EPOCH,
            ended_at: DateTime::UNIX_EPOCH,
            repository: None,
            prompts,
            tool_calls: 0,
            models: Vec::new(),
        }
    }

    #[test]
    fn ais_019_only_a_session_day_whose_counts_changed_is_handed_over_again() {
        let mut watch = UsageWatch::new();
        assert_eq!(watch.changed(vec![day(1)]), [day(1)], "the first read");
        assert_eq!(watch.changed(vec![day(1)]).len(), 0, "unchanged");
        assert_eq!(watch.changed(vec![day(2)]), [day(2)], "a new prompt");
    }
}
