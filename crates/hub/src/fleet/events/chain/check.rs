//! LOG-027. Pure: the store feeds the walk one link at a time, in sequence order.

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use super::link::{FIRST_PREVIOUS, Hash, Link, StoredEntry, hash};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::fleet::events) struct ChainCheck {
    pub links: u64,
    /// Links whose entry the retention dropped.
    pub dropped: u64,
    pub head: Option<(i64, Hash)>,
    pub fault: Option<Fault>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::fleet::events) struct Fault {
    pub kind: FaultKind,
    /// `None` for an entry that has no link.
    pub seq: Option<i64>,
    pub entry_id: Uuid,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::events) enum FaultKind {
    /// A field changed, or entries changed places.
    Changed,
    Missing,
    Broken,
    /// The hub did not write the entry.
    NotLinked,
}

pub(in crate::fleet::events) struct Walk {
    first_kept_day: NaiveDate,
    links: u64,
    dropped: u64,
    head: Option<(i64, Hash)>,
    fault: Option<Fault>,
}

impl Walk {
    pub(in crate::fleet::events) fn new(first_kept_day: NaiveDate) -> Self {
        Self {
            first_kept_day,
            links: 0,
            dropped: 0,
            head: None,
            fault: None,
        }
    }

    /// `false` at the first fault: the walk then reads no more links.
    pub(in crate::fleet::events) fn step(
        &mut self,
        link: &Link,
        entry: Option<&StoredEntry>,
    ) -> bool {
        let (seq, previous) = self
            .head
            .map_or((1, FIRST_PREVIOUS), |(seq, hash)| (seq + 1, hash));
        let kind = if link.seq != seq || link.previous != previous {
            Some(FaultKind::Broken)
        } else {
            self.entry_fault(link, entry)
        };
        if let Some(kind) = kind {
            self.fault = Some(Fault {
                kind,
                seq: Some(link.seq),
                entry_id: link.entry_id,
                at: link.entry_at,
            });
            return false;
        }
        self.links += 1;
        self.head = Some((link.seq, link.hash));
        true
    }

    fn entry_fault(&mut self, link: &Link, entry: Option<&StoredEntry>) -> Option<FaultKind> {
        match entry {
            Some(entry) if hash(link.seq, &link.previous, entry) == link.hash => None,
            Some(_) => Some(FaultKind::Changed),
            None if link.entry_at.date_naive() < self.first_kept_day => {
                self.dropped += 1;
                None
            }
            None => Some(FaultKind::Missing),
        }
    }

    pub(in crate::fleet::events) fn finish(
        self,
        unlinked: Option<(Uuid, DateTime<Utc>)>,
    ) -> ChainCheck {
        let fault = self.fault.or_else(|| {
            unlinked.map(|(entry_id, at)| Fault {
                kind: FaultKind::NotLinked,
                seq: None,
                entry_id,
                at,
            })
        });
        ChainCheck {
            links: self.links,
            dropped: self.dropped,
            head: self.head,
            fault,
        }
    }
}
