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
    /// Settled at `now`, which ends a snooze and forgets a wake; or taken back to the active threads at `now`. To
    /// take back a thread that is not settled changes nothing.
    #[must_use]
    pub const fn settled(self, on: bool, now: DateTime<Utc>) -> Self {
        if !on && self.settled_at.is_none() {
            self
        } else if on {
            Self {
                settled_at: Some(now),
                snoozed_at: None,
                snoozed_until: None,
                woke_at: None,
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

    /// Snoozed at `now` until `until`, which takes the thread off the shelf Settled. A pinned
    /// thread can be snoozed: it leaves the pinned group until it wakes.
    #[must_use]
    pub const fn snoozed(self, until: DateTime<Utc>, now: DateTime<Utc>) -> Self {
        Self {
            settled_at: None,
            woke_at: None,
            snoozed_at: Some(now),
            snoozed_until: Some(until),
            ..self
        }
    }

    /// Woken at `now`: no snooze. To wake a thread that is not snoozed changes nothing.
    #[must_use]
    pub const fn woken(self, now: DateTime<Utc>) -> Self {
        if self.snoozed_until.is_none() {
            return self;
        }
        Self {
            snoozed_at: None,
            snoozed_until: None,
            woke_at: Some(now),
            ..self
        }
    }

    /// Without the marks that can no longer change the shelf at `now` or later: a take-back, a
    /// wake or a snooze that ended more than [`SETTLE_AFTER`] ago. So a listing with only such
    /// marks is plain again and leaves the record.
    #[must_use]
    pub fn tidied(self, now: DateTime<Utc>) -> Self {
        let recent = |at: &DateTime<Utc>| now - *at < SETTLE_AFTER;
        let snooze_holds = self
            .snoozed_until
            .is_some_and(|until| recent(&until) || until > now);
        Self {
            unsettled_at: self.unsettled_at.filter(recent),
            woke_at: self.woke_at.filter(recent),
            snoozed_at: self.snoozed_at.filter(|_| snooze_holds),
            snoozed_until: self.snoozed_until.filter(|_| snooze_holds),
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
mod tests;
