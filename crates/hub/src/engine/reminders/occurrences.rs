//! The occurrences the engine gives with no extension, and the interface of an extension that gives its own (K-E6 Section 1; ENG-001).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use super::model::{Class, Occurrence};
use crate::{
    engine::{records::Reference, sources::Health},
    fleet::{
        credentials::operational::{OperationalKey, VersionState},
        events::EventSummary,
    },
    ids::SourceAccountId,
};

/// An event that starts with these gives no occurrence, so a rule about the hub's events does not fire on its own reminders.
pub(super) const OWN_EVENTS: [&str; 2] = ["The reminder ", "A reminder was sent."];
const KEY_EXPIRY: &str = "keys.expiry";
const KEY_ROTATION: &str = "keys.rotation";
const TITLE_MAX: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the {0} extension could not give its occurrences")]
pub struct Unavailable(pub &'static str);

/// What an extension gives on the sweep, for what has no moment of storage; an extension that is off is not read.
#[async_trait]
pub trait Triggers: Send + Sync {
    /// The key of the extension (K-E7).
    fn name(&self) -> &'static str;

    /// The things that end at a time.
    async fn expiries(
        &self,
        db: &PgPool,
        now: DateTime<Utc>,
    ) -> Result<Vec<Occurrence>, Unavailable>;

    /// The numbers at a time.
    async fn measures(
        &self,
        db: &PgPool,
        now: DateTime<Utc>,
    ) -> Result<Vec<Occurrence>, Unavailable>;
}

pub type Extensions = Vec<Arc<dyn Triggers>>;

fn one_line(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(TITLE_MAX)
        .collect()
}

pub(super) fn of_event(event: &EventSummary) -> Option<Occurrence> {
    if OWN_EVENTS.iter().any(|own| event.message.starts_with(own)) {
        return None;
    }
    Some(Occurrence::Record {
        source: event.source.as_str().to_owned(),
        kind: event.severity.as_str().to_owned(),
        record: Reference::parse(&format!("event:{}", event.id)).ok()?,
        title: one_line(&event.message),
        class: Class::C,
        at: event.at,
    })
}

pub(super) fn of_keys(keys: &[OperationalKey]) -> Vec<Occurrence> {
    let mut found = Vec::new();
    for key in keys {
        let Ok(record) = Reference::parse(&format!("operational_key:{}", key.id)) else {
            continue;
        };
        let mut push = |subject: &str, expires_at| {
            found.push(Occurrence::Expiry {
                subject: subject.to_owned(),
                record: record.clone(),
                title: one_line(&key.name),
                class: Class::C,
                expires_at,
            });
        };
        if let Some(expires_at) = key.metadata.expires_at {
            push(KEY_EXPIRY, expires_at);
        }
        let active = key
            .versions
            .iter()
            .find(|version| version.state == VersionState::Active);
        if let (Some(days), Some(active)) = (key.metadata.rotation_days, active) {
            push(
                KEY_ROTATION,
                active.created_at + TimeDelta::days(i64::from(days)),
            );
        }
    }
    found
}

/// Every other health is `other`, which no rule waits for (K-E6 Section 2).
const fn state_word(health: Health) -> &'static str {
    match health {
        Health::Ok => "connected",
        Health::Stalled => "stalled",
        Health::NeedsReconnect => "needs_reconnect",
        _ => "other",
    }
}

pub(super) struct Seen {
    pub account: String,
    pub label: String,
    pub state: &'static str,
}

pub(super) fn seen(healths: Vec<(SourceAccountId, String, Health)>) -> Vec<Seen> {
    healths
        .into_iter()
        .map(|(id, label, health)| Seen {
            account: id.to_string(),
            label,
            state: state_word(health),
        })
        .collect()
}

/// An account that the sweep sees for the first time gives no occurrence.
pub(super) fn of_health(
    before: &[(String, String)],
    now_seen: &[Seen],
    at: DateTime<Utc>,
) -> Vec<Occurrence> {
    now_seen
        .iter()
        .filter(|seen| {
            before
                .iter()
                .any(|(account, state)| *account == seen.account && state != seen.state)
        })
        .filter_map(|seen| {
            Some(Occurrence::Record {
                source: "sources".to_owned(),
                kind: seen.state.to_owned(),
                record: Reference::parse(&format!("source_account:{}", seen.account)).ok()?,
                title: one_line(&seen.label),
                class: Class::C,
                at,
            })
        })
        .collect()
}
