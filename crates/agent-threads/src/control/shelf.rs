//! Where the list shows a thread (TH61; inventory 1.4, 1.6): with the active threads, on the
//! shelf Snoozed until a time, or on the shelf Settled. The person settles, un-settles, snoozes
//! and wakes a thread; the runner keeps when, as [`Marks`] in the listing. A thread that nobody
//! asked anything for [`SETTLE_AFTER`] settles by itself: that is worked out from the marks and
//! the work of the thread each time the list is read, so no timer of the runner changes the
//! record.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::Listing;

/// A thread with no work and no prompt for this time settles by itself (T3 Code's default).
pub const SETTLE_AFTER: TimeDelta = TimeDelta::days(7);

/// The longest snooze (T3 Code's longest preset).
pub const MOST_SNOOZE: TimeDelta = TimeDelta::days(30);

/// When the person last settled, un-settled, snoozed or woke a thread. Each is absent until then.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<DateTime<Utc>>,
    /// The person took a settled thread back: the week of [`SETTLE_AFTER`] starts again here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsettled_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<DateTime<Utc>>,
    /// The person ended a snooze before its time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub woke_at: Option<DateTime<Utc>>,
}

impl Marks {
    /// Settled at `now`, which ends a snooze; or taken back to the active threads at `now`.
    #[must_use]
    pub const fn settled(self, on: bool, now: DateTime<Utc>) -> Self {
        if on {
            Self {
                settled_at: Some(now),
                snoozed_at: None,
                snoozed_until: None,
                ..self
            }
        } else {
            Self {
                settled_at: None,
                unsettled_at: Some(now),
                ..self
            }
        }
    }

    /// Snoozed at `now` until `until`, which takes the thread off the shelf Settled.
    #[must_use]
    pub const fn snoozed(self, until: DateTime<Utc>, now: DateTime<Utc>) -> Self {
        Self {
            settled_at: None,
            snoozed_at: Some(now),
            snoozed_until: Some(until),
            ..self
        }
    }

    /// Woken at `now`: no snooze.
    #[must_use]
    pub const fn woken(self, now: DateTime<Utc>) -> Self {
        Self {
            snoozed_at: None,
            snoozed_until: None,
            woke_at: Some(now),
            ..self
        }
    }
}

/// What the list knows of the work of a thread, from its files or from the runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activity {
    /// The newest prompt of the person, or the start of the thread.
    pub prompted: DateTime<Utc>,
    /// The newest moment of work: a prompt, a reply or the end of a turn.
    pub last: DateTime<Utc>,
    /// The program works now.
    pub working: bool,
    /// The newest request that waits for the person, if one waits.
    pub asked: Option<DateTime<Utc>>,
}

/// Where the list shows a thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shelf", rename_all = "snake_case")]
pub enum Shelf {
    Active,
    Snoozed {
        until: DateTime<Utc>,
    },
    /// `auto` when no person settled it: it was quiet for [`SETTLE_AFTER`] from `since`.
    Settled {
        since: DateTime<Utc>,
        auto: bool,
    },
}

impl Listing {
    /// Where the list shows this thread at `now`. A prompt after the person settled the thread
    /// takes it back; a prompt or a request after a snooze started ends the snooze; a pinned or
    /// archived thread, or one that works or waits for the person, never settles by itself.
    #[must_use]
    pub fn shelf(&self, activity: &Activity, now: DateTime<Utc>) -> Shelf {
        let marks = &self.marks;
        if let Some(since) = marks.settled_at.filter(|at| activity.prompted <= *at) {
            return Shelf::Settled { since, auto: false };
        }
        if let Some(until) = snoozed_until(marks, activity, now) {
            return Shelf::Snoozed { until };
        }
        let holds = self.pinned || self.archived || activity.working || activity.asked.is_some();
        let since = quiet_since(marks, activity, now);
        if !holds && now - since >= SETTLE_AFTER {
            return Shelf::Settled { since, auto: true };
        }
        Shelf::Active
    }
}

/// The end of the snooze that holds at `now`, if one holds.
fn snoozed_until(marks: &Marks, activity: &Activity, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let until = marks.snoozed_until.filter(|until| *until > now)?;
    let from = marks.snoozed_at?;
    let woken = activity.prompted > from || activity.asked.is_some_and(|asked| asked > from);
    (!woken).then_some(until)
}

/// The start of the quiet that auto-settle measures: the newest work, or the newest time the
/// person took the thread back, woke it or a snooze ran out.
fn quiet_since(marks: &Marks, activity: &Activity, now: DateTime<Utc>) -> DateTime<Utc> {
    let ran_out = marks.snoozed_until.filter(|until| *until <= now);
    [
        marks.unsettled_at,
        marks.woke_at,
        ran_out,
        Some(activity.prompted),
    ]
    .into_iter()
    .flatten()
    .fold(activity.last, DateTime::max)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::control::ListingChange;

    fn at(hours: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 0).expect("a time") + TimeDelta::hours(hours)
    }

    fn quiet(since: i64) -> Activity {
        Activity {
            prompted: at(since),
            last: at(since),
            working: false,
            asked: None,
        }
    }

    fn listing(changes: &[(ListingChange, i64)]) -> Listing {
        let plain = Listing {
            thread: Uuid::nil(),
            ..Listing::default()
        };
        changes.iter().fold(plain, |listing, (change, hour)| {
            listing.changed(change, at(*hour)).expect("a change")
        })
    }

    const WEEK: i64 = 7 * 24;

    #[test]
    fn th61_a_settled_thread_stays_settled_until_the_person_prompts_it() {
        let settled = listing(&[(ListingChange::Settle { on: true }, 2)]);
        let shelf = Shelf::Settled {
            since: at(2),
            auto: false,
        };
        assert_eq!(settled.shelf(&quiet(1), at(3)), shelf);
        assert_eq!(
            settled.shelf(&quiet(4), at(5)),
            Shelf::Active,
            "a new prompt"
        );
        let back = listing(&[
            (ListingChange::Settle { on: true }, 2),
            (ListingChange::Settle { on: false }, 3),
        ]);
        assert_eq!(back.shelf(&quiet(1), at(4)), Shelf::Active);
        assert!(!back.is_plain(), "the record keeps when it was taken back");
    }

    #[test]
    fn th61_a_snooze_ends_at_its_time_on_a_wake_a_prompt_or_a_request() {
        let snoozed = listing(&[(ListingChange::Snooze { until: at(10) }, 2)]);
        let until = Shelf::Snoozed { until: at(10) };
        assert_eq!(snoozed.shelf(&quiet(1), at(9)), until);
        assert_eq!(snoozed.shelf(&quiet(1), at(10)), Shelf::Active, "its time");
        assert_eq!(snoozed.shelf(&quiet(3), at(4)), Shelf::Active, "a prompt");
        let asked = Activity {
            asked: Some(at(3)),
            ..quiet(1)
        };
        assert_eq!(snoozed.shelf(&asked, at(4)), Shelf::Active, "a request");
        let old_ask = Activity {
            asked: Some(at(1)),
            ..quiet(1)
        };
        assert_eq!(
            snoozed.shelf(&old_ask, at(4)),
            until,
            "a request from before"
        );
        let woken = listing(&[
            (ListingChange::Snooze { until: at(10) }, 2),
            (ListingChange::Wake, 3),
        ]);
        assert_eq!(woken.shelf(&quiet(1), at(4)), Shelf::Active);
    }

    #[test]
    fn th61_a_snooze_must_end_after_now_and_within_thirty_days() {
        let plain = listing(&[]);
        for until in [at(0), at(-1), at(31 * 24)] {
            let snooze = ListingChange::Snooze { until };
            assert_eq!(
                plain.clone().changed(&snooze, at(0)),
                Err(super::super::BadChange::SnoozeTime)
            );
        }
    }

    #[test]
    fn th61_a_quiet_thread_settles_by_itself_after_a_week() {
        let plain = listing(&[]);
        assert_eq!(plain.shelf(&quiet(0), at(WEEK - 1)), Shelf::Active);
        let auto = Shelf::Settled {
            since: at(0),
            auto: true,
        };
        assert_eq!(plain.shelf(&quiet(0), at(WEEK)), auto);
        let pinned = listing(&[(ListingChange::Pin { on: true }, 0)]);
        assert_eq!(pinned.shelf(&quiet(0), at(WEEK)), Shelf::Active, "pinned");
        let waits = Activity {
            asked: Some(at(0)),
            ..quiet(0)
        };
        assert_eq!(
            plain.shelf(&waits, at(WEEK)),
            Shelf::Active,
            "a request waits"
        );
        let taken_back = listing(&[(ListingChange::Settle { on: false }, 5)]);
        assert_eq!(
            taken_back.shelf(&quiet(0), at(WEEK)),
            Shelf::Active,
            "taken back"
        );
        let ran_out = listing(&[(ListingChange::Snooze { until: at(24) }, 1)]);
        assert_eq!(
            ran_out.shelf(&quiet(0), at(WEEK + 1)),
            Shelf::Active,
            "the week starts when the snooze ran out"
        );
    }

    #[test]
    fn th61_the_record_reads_back_what_it_wrote() {
        let marked = listing(&[(ListingChange::Snooze { until: at(10) }, 2)]);
        let text = serde_json::to_string(&marked).expect("json");
        assert!(text.contains("\"snoozed_until\""), "{text}");
        assert!(
            !text.contains("settled_at"),
            "an absent mark is not written: {text}"
        );
        let back: Listing = serde_json::from_str(&text).expect("read");
        assert_eq!(back, marked);
        let old: Listing = serde_json::from_str(&format!(
            "{{\"thread\":\"{}\",\"pinned\":true}}",
            Uuid::nil()
        ))
        .expect("a record from before TH61");
        assert_eq!(old.marks, Marks::default());
    }
}
