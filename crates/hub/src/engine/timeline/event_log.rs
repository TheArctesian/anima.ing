//! Reads the log through the events domain and stores nothing (TML-007).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    adapter::{Adapter, Unavailable},
    model::{Ask, Basis, Category, Evidence, Mark},
};
use crate::{
    engine::records::Reference,
    fleet::events::{self, EventSummary, Source},
};

pub(super) const KIND: &str = "event";
const EVENT_PREFIX: &str = "event:";
const NAME: &str = "event_log";
/// The hub saw these events itself.
const RANK: u16 = 100;
/// An event is one instant, so every entry carries this note (TML-006).
const NO_END: &str = "End time not given by the source.";

struct EventLog;

#[must_use]
pub fn event_log() -> Arc<dyn Adapter> {
    Arc::new(EventLog)
}

fn category(source: Source) -> Category {
    if source == Source::Session {
        Category::Session
    } else {
        Category::Device
    }
}

fn sources(categories: Option<&[Category]>) -> Option<Vec<Source>> {
    let wanted = categories?;
    let all = [
        Source::Check,
        Source::Session,
        Source::Device,
        Source::Access,
        Source::Hub,
        Source::Metrics,
        Source::Notifications,
        Source::Vpn,
    ];
    Some(
        all.into_iter()
            .filter(|source| wanted.contains(&category(*source)))
            .collect(),
    )
}

/// Events sort by instant, then ID; a mark that is not an event sorts by its reference text.
fn before(ask: &Ask) -> (DateTime<Utc>, Uuid) {
    let window_end = (ask.to, Uuid::nil());
    let Some(Mark { start, reference }) = &ask.after else {
        return window_end;
    };
    if *start >= ask.to {
        return window_end;
    }
    let id = if reference.kind() == KIND {
        reference.id().parse().unwrap_or(Uuid::nil())
    } else if reference.as_str() > EVENT_PREFIX {
        Uuid::max()
    } else {
        Uuid::nil()
    };
    (*start, id)
}

fn evidence(event: EventSummary) -> Option<Evidence> {
    let title = match &event.device {
        Some((_, device)) => format!("{device}: {}", event.message),
        None => event.message,
    };
    Some(Evidence {
        reference: Reference::new(KIND, &event.id.to_string()).ok()?,
        source: NAME.to_owned(),
        rank: RANK,
        category: category(event.source),
        title,
        start: event.at,
        end: None,
        received_at: None,
        basis: Basis::Observed,
        uncertainty: vec![NO_END.to_owned()],
        occurrence: None,
        superseded_by: None,
    })
}

#[async_trait]
impl Adapter for EventLog {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn evidence(&self, db: &PgPool, ask: &Ask) -> Result<Vec<Evidence>, Unavailable> {
        let sources = sources(ask.categories.as_deref());
        if sources.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(ask.limit).unwrap_or(i64::MAX);
        let events = events::before(db, (ask.from, before(ask)), sources, limit)
            .await
            .map_err(|cause| Unavailable {
                adapter: NAME,
                cause: Box::new(cause),
            })?;
        Ok(events.into_iter().filter_map(evidence).collect())
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;

    use super::*;

    fn ask(after: Option<(&str, u32)>) -> Ask {
        let hour = |h| Utc.with_ymd_and_hms(2030, 1, 1, h, 0, 0).unwrap();
        Ask {
            from: hour(0),
            to: hour(10),
            after: after.map(|(reference, h)| Mark {
                start: hour(h),
                reference: Reference::parse(reference).unwrap(),
            }),
            categories: None,
            limit: 10,
        }
    }

    #[test]
    fn tml_007_a_page_continues_at_the_mark_whatever_record_the_mark_names() {
        let id = "0192f5a0-7c3e-7a52-9d1b-3f4e5a6b7c8d";
        let hour = |h| Utc.with_ymd_and_hms(2030, 1, 1, h, 0, 0).unwrap();
        assert_eq!(before(&ask(None)), (hour(10), Uuid::nil()));
        assert_eq!(
            before(&ask(Some((&format!("event:{id}"), 5)))),
            (hour(5), id.parse().unwrap())
        );
        assert_eq!(before(&ask(Some(("workout:1", 5)))), (hour(5), Uuid::max()));
        assert_eq!(before(&ask(Some(("book:1", 5)))), (hour(5), Uuid::nil()));
        assert_eq!(before(&ask(Some(("event1:1", 5)))), (hour(5), Uuid::nil()));
        assert_eq!(before(&ask(Some(("book:1", 12)))), (hour(10), Uuid::nil()));
    }

    #[test]
    fn tml_004_a_category_filter_reads_only_the_event_sources_of_that_category() {
        assert_eq!(sources(None), None);
        assert_eq!(
            sources(Some(&[Category::Session])),
            Some(vec![Source::Session])
        );
        assert_eq!(sources(Some(&[Category::Sleep])), Some(Vec::new()));
        let device = sources(Some(&[Category::Device])).unwrap();
        assert_eq!(device.len(), 7);
        assert!(!device.contains(&Source::Session));
    }
}
