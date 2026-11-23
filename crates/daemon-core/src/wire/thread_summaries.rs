//! Thread summaries carried by the daemon's metrics push (K8 Section 4, TH20).

pub(super) mod entries;

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_usage::{Source, is_repository};

const MOST_SUMMARIES: usize = 256;
const MAX_REVISION: i64 = (1 << 53) - 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummaries {
    pub count: u32,
    pub count_revision: i64,
    pub summaries: Vec<ThreadSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub runner_id: Uuid,
    pub program: String,
    pub state: String,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub activity_at: DateTime<Utc>,
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummariesReceipt {
    pub count_revision: i64,
    pub summaries: Vec<ThreadSummaryReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummaryReceipt {
    pub runner_id: Uuid,
    pub id: Uuid,
    pub revision: i64,
}

impl ThreadSummaries {
    pub(super) fn fits(&self) -> bool {
        let mut seen = BTreeSet::new();
        (1..=MAX_REVISION).contains(&self.count_revision)
            && self.summaries.len() <= MOST_SUMMARIES
            && self
                .summaries
                .iter()
                .all(|row| row.fits() && seen.insert(row.runner_id))
    }
}

impl ThreadSummary {
    pub(super) fn fits(&self) -> bool {
        !self.runner_id.is_nil()
            && Source::parse(&self.program).is_some()
            && matches!(
                self.state.as_str(),
                "starting" | "working" | "needs_you" | "idle" | "stopped" | "unknown"
            )
            && self.title.chars().count() <= 80
            && !self.title.chars().any(char::is_control)
            && self.started_at <= self.activity_at
            && (1..=MAX_REVISION).contains(&self.revision)
            && self.repository.as_deref().is_none_or(is_repository)
    }
}
