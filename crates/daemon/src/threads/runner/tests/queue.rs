//! The queue of a thread (TH109): prompts go one by one, each after a turn that ended well.

use agent_threads::control::{Change, LastTurn, MOST_QUEUED, Refusal};
use uuid::Uuid;

use super::{codex_in_a_turn, counter, held_claude, writes};
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
        thread
            .send(&agent_threads::drive::Prompt::plain("now"), true)
            .expect("sent");
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

#[test]
fn th109_a_prompt_sent_during_a_turn_keeps_the_queue_one_turn_behind() {
    // Claude Code holds a prompt sent during a turn and runs it as a turn of its own.
    let mut thread = held_claude();
    queue(&mut thread, 1, "queued");
    thread
        .send(&agent_threads::drive::Prompt::plain("steer"), true)
        .expect("sent");
    assert_eq!(prompts(&line(&mut thread, DONE)), NONE);
    // The held prompt fails: the queue stops, and the queued prompt never went.
    assert_eq!(prompts(&line(&mut thread, FAILED)), NONE);
    assert_eq!(thread.row.queued.len(), 1);
}

#[test]
fn th109_the_late_end_of_a_stopped_turn_does_not_stop_the_queue_again() {
    let mut thread = held_claude();
    queue(&mut thread, 1, "queued");
    thread.interrupt();
    thread
        .send(&agent_threads::drive::Prompt::plain("instead"), true)
        .expect("sent");
    assert_eq!(prompts(&line(&mut thread, STOPPED)), NONE);
    assert_eq!(prompts(&line(&mut thread, DONE)), ["queued"]);
}

#[test]
fn th109_a_prompt_of_an_agent_does_not_start_a_stopped_queue() {
    let mut thread = held_claude();
    queue(&mut thread, 1, "queued");
    line(&mut thread, FAILED);
    thread
        .send(&agent_threads::drive::Prompt::plain("from an agent"), false)
        .expect("sent");
    assert_eq!(prompts(&line(&mut thread, DONE)), NONE);
}

#[test]
fn th109_an_empty_or_too_long_prompt_is_refused() {
    let mut thread = held_claude();
    assert_eq!(
        thread.queue(Uuid::from_u128(1), " ".to_owned()),
        Err(Refusal::BadValue)
    );
    let long = "x".repeat(agent_threads::control::MOST_QUEUED_BYTES + 1);
    assert_eq!(
        thread.queue(Uuid::from_u128(1), long),
        Err(Refusal::BadValue)
    );
}

const CODEX_DONE: &str =
    r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#;

#[test]
fn th109_a_codex_steer_ends_with_its_turn_and_a_refused_turn_stops_the_queue() {
    let mut ids = counter();
    let (mut thread, _) = codex_in_a_turn(&mut ids);
    queue(&mut thread, 1, "first");
    queue(&mut thread, 2, "second");
    // A steer goes into the running turn: it does not hold the queue back.
    thread
        .send(&agent_threads::drive::Prompt::plain("steer"), true)
        .expect("sent");
    let sent = writes(&thread.line(CODEX_DONE, &mut ids));
    assert_eq!(sent[0]["method"], "turn/start");
    assert_eq!(sent[0]["params"]["input"][0]["text"], "first");
    // Codex refuses that turn: it ends as failed, the person is told, and "second" waits.
    let refusal = format!(
        r#"{{"id":{},"error":{{"code":-32600,"message":"busy"}}}}"#,
        sent[0]["id"]
    );
    let steps = thread.line(&refusal, &mut ids);
    assert_eq!(writes(&steps).len(), 0);
    assert_eq!(thread.row.last_turn, Some(LastTurn::Failed));
    assert_eq!(thread.row.queued.len(), 1);
}

#[test]
fn th109_two_prompts_sent_before_the_program_says_it_runs_are_two_turns() {
    let mut thread = held_claude();
    line(&mut thread, DONE);
    line(&mut thread, IDLE);
    // The program still says Idle when the person sends twice: each prompt is a turn.
    thread
        .send(&agent_threads::drive::Prompt::plain("a"), true)
        .expect("sent");
    thread
        .send(&agent_threads::drive::Prompt::plain("b"), true)
        .expect("sent");
    queue(&mut thread, 1, "later");
    assert_eq!(
        prompts(&line(&mut thread, DONE)),
        NONE,
        "the queued prompt waits for the turn of b"
    );
    assert_eq!(prompts(&line(&mut thread, DONE)), ["later"]);
}
