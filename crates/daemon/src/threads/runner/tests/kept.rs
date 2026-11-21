//! What the runner keeps of a thread and gives back (TH101, TH106).

use agent_threads::control::Change;
use agent_threads::drive::DriveEvent;

use super::claude;

fn diff(turn: &str, diff: &str) -> Change {
    Change::Said {
        event: DriveEvent::TurnDiff {
            turn: (!turn.is_empty()).then(|| turn.to_owned()),
            diff: diff.to_owned(),
        },
    }
}

fn said(text: &str) -> Change {
    Change::Said {
        event: DriveEvent::TextDelta {
            text: text.to_owned(),
        },
    }
}

#[test]
fn th101_only_the_newest_diff_of_a_turn_is_kept() {
    let mut thread = claude();
    thread.note(diff("", "no turn"));
    thread.note(diff("a", "one"));
    thread.note(said("between"));
    thread.note(diff("b", "other turn"));
    thread.note(diff("a", "one and two"));
    let kept: Vec<Change> = thread.kept().map(|update| update.change.clone()).collect();
    assert_eq!(
        kept,
        vec![
            diff("", "no turn"),
            said("between"),
            diff("b", "other turn"),
            diff("a", "one and two")
        ]
    );
}

#[test]
fn th101_the_bytes_of_a_dropped_diff_are_given_back() {
    let mut thread = claude();
    thread.note(diff("a", &"x".repeat(1000)));
    thread.note(diff("a", "short"));
    let newest: Vec<Change> = thread
        .kept_newest(10, 200)
        .map(|update| update.change.clone())
        .collect();
    assert_eq!(newest, vec![diff("a", "short")]);
}

#[test]
fn th106_a_read_back_gives_one_update_however_big_it_is() {
    let mut thread = claude();
    thread.note(said("small"));
    let big = thread.note(said(&"x".repeat(4096)));
    let next = thread.note(said("after"));
    let back: Vec<u64> = thread
        .kept_before(next.seq, 10, 100)
        .map(|update| update.seq)
        .collect();
    assert_eq!(back, vec![big.seq]);
}
