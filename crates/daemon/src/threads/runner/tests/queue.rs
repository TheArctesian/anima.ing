//! The queue of a thread (TH109): prompts go one by one, each after a turn that ended well.

use agent_threads::control::{Change, LastTurn, MOST_QUEUED, Refusal};
use uuid::Uuid;

use super::{counter, held_claude, writes};
use crate::threads::runner::thread::{Step, Thread};

const DONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","terminal_reason":"completed"}"#;
const FAILED: &str =
    r#"{"type":"result","subtype":"error_max_turns","terminal_reason":"max_turns"}"#;
const STOPPED: &str = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming"}"#;
const IDLE: &str = r#"{"type":"system","subtype":"session_state_changed","state":"idle"}"#;

const NONE: [String; 0] = [];

fn line(thread: &mut Thread, text: &str) -> Vec<Step> {
    thread.line(text, &mut counter())
}

/// The prompts that the steps send to the program.
fn prompts(steps: &[Step]) -> Vec<String> {
    writes(steps)
        .iter()
        .filter_map(|line| line["message"]["content"].as_str().map(str::to_owned))
        .collect()
}

fn queue(thread: &mut Thread, n: u128, text: &str) -> Vec<Step> {
    thread
        .queue(Uuid::from_u128(n), text.to_owned())
        .expect("queued")
}

#[test]
fn th109_queued_prompts_go_one_by_one_each_after_a_turn_that_ended_well() {
    let mut thread = held_claude();
    assert_eq!(prompts(&queue(&mut thread, 1, "first")), NONE);
    assert_eq!(prompts(&queue(&mut thread, 2, "second")), NONE);
    assert_eq!(thread.row.queued.len(), 2);

    assert_eq!(prompts(&line(&mut thread, DONE)), ["first"]);
    // The program says that it is idle before the queued turn starts: the second waits.
    assert_eq!(prompts(&line(&mut thread, IDLE)), NONE);
    assert_eq!(thread.row.queued.len(), 1);
    assert_eq!(prompts(&line(&mut thread, DONE)), ["second"]);
    assert_eq!(thread.row.queued.len(), 0);
    assert_eq!(thread.row.last_turn, Some(LastTurn::Done));
}

#[test]
fn th109_a_failed_or_stopped_turn_stops_the_queue_until_the_person_sends() {
    for end in [FAILED, STOPPED] {
        let mut thread = held_claude();
        queue(&mut thread, 1, "next");
        assert_eq!(prompts(&line(&mut thread, end)), NONE, "{end}");
        assert_eq!(thread.row.queued.len(), 1);
        assert_ne!(thread.row.last_turn, Some(LastTurn::Done));
        // The person sends: after that turn the queue goes again.
        thread.send("now").expect("sent");
        assert_eq!(prompts(&line(&mut thread, DONE)), ["next"]);
    }
}

#[test]
fn th109_a_prompt_queued_while_the_program_waits_goes_at_once() {
    let mut thread = held_claude();
    line(&mut thread, DONE);
    assert_eq!(prompts(&queue(&mut thread, 1, "go on")), ["go on"]);
    assert_eq!(thread.row.queued.len(), 0);
}

#[test]
fn th109_the_queue_is_bounded_and_a_gone_prompt_is_refused() {
    let mut thread = held_claude();
    for n in 0..MOST_QUEUED {
        queue(&mut thread, n as u128 + 1, "p");
    }
    assert_eq!(
        thread.queue(Uuid::from_u128(99), "p".to_owned()),
        Err(Refusal::QueueFull)
    );
    let gone = Uuid::from_u128(99);
    assert_eq!(
        thread.requeue(gone, "x".to_owned()),
        Err(Refusal::RequestGone)
    );
    assert_eq!(thread.unqueue(gone).err(), Some(Refusal::RequestGone));
    thread
        .requeue(Uuid::from_u128(1), "new".to_owned())
        .expect("requeued");
    assert_eq!(thread.row.queued[0].text, "new");
    let (text, step) = thread.take_queued(Uuid::from_u128(1)).expect("taken");
    assert_eq!(text, "new");
    assert!(matches!(step, Step::Change(Change::State { .. })));
}

#[test]
fn th109_the_record_never_holds_the_queue_and_a_stop_empties_it() {
    let mut thread = held_claude();
    queue(&mut thread, 1, "a secret plan");
    assert!(
        !serde_json::to_string(&thread.saved())
            .expect("json")
            .contains("a secret plan")
    );
    assert!(!thread.record_key().contains("a secret plan"));
    thread.exited(Some(0), None);
    assert_eq!(thread.row.queued.len(), 0);
    assert_eq!(
        thread.queue(Uuid::from_u128(2), "late".to_owned()),
        Err(Refusal::NotRunning)
    );
}
