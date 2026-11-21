//! The rule of the shelf (TH61): the same cases as `lib/agent-threads/listing/shelf.test.ts`.

use uuid::Uuid;

use super::*;
use crate::control::{BadChange, ListingChange};

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
    let again = listing(&[
        (ListingChange::Snooze { until: at(10) }, 2),
        (ListingChange::Wake, 3),
        (ListingChange::Snooze { until: at(20) }, 4),
    ]);
    assert_eq!(again.marks.woke_at, None, "a new snooze forgets the wake");
    let pinned = listing(&[
        (ListingChange::Pin { on: true }, 0),
        (ListingChange::Snooze { until: at(10) }, 1),
    ]);
    let snoozed_pin = Shelf::Snoozed { until: at(10) };
    assert_eq!(
        pinned.shelf(&quiet(0), at(2)),
        snoozed_pin,
        "a pinned thread snoozes"
    );
}

#[test]
fn th61_a_snooze_must_end_after_now_and_within_thirty_days() {
    let plain = listing(&[]);
    for until in [at(0), at(-1), at(31 * 24)] {
        let snooze = ListingChange::Snooze { until };
        assert_eq!(
            plain.clone().changed(&snooze, at(0)),
            Err(BadChange::SnoozeTime)
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
    let taken_back = listing(&[
        (ListingChange::Settle { on: true }, 4),
        (ListingChange::Settle { on: false }, 5),
    ]);
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
fn th61_a_mark_that_no_longer_counts_leaves_the_record() {
    assert!(
        listing(&[(ListingChange::Wake, 0)]).is_plain(),
        "nothing to wake"
    );
    assert!(
        listing(&[(ListingChange::Settle { on: false }, 0)]).is_plain(),
        "nothing to take back"
    );
    let old = listing(&[
        (ListingChange::Settle { on: true }, 0),
        (ListingChange::Settle { on: false }, 1),
        (ListingChange::Snooze { until: at(3) }, 2),
        (ListingChange::Pin { on: true }, 3 + WEEK),
        (ListingChange::Pin { on: false }, 3 + WEEK),
    ]);
    assert!(old.is_plain(), "{old:?}");
    let fresh = listing(&[
        (ListingChange::Snooze { until: at(3) }, 2),
        (ListingChange::Pin { on: false }, 4),
    ]);
    assert_eq!(
        fresh.marks.snoozed_until,
        Some(at(3)),
        "it still starts the week"
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
