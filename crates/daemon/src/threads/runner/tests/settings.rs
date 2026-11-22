//! A change of the model, the effort or the mode after the start (TH110): from the next turn.

use agent_threads::control::{Change, Refusal};
use agent_threads::drive::{DriveEvent, Effort, Mode, Model};

use super::{claude, codex_in_a_turn, counter, held_claude, writes};
use crate::threads::runner::thread::{Changes, Step, Thread};

const DONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","terminal_reason":"completed"}"#;
const IDLE: &str = r#"{"type":"system","subtype":"session_state_changed","state":"idle"}"#;

fn model(name: &str) -> Model {
    Model::new(name).expect("a model")
}

fn subtypes(steps: &[Step]) -> Vec<String> {
    writes(steps)
        .iter()
        .filter_map(|line| line["request"]["subtype"].as_str().map(str::to_owned))
        .collect()
}

fn idle_claude() -> Thread {
    let mut thread = held_claude();
    thread.line(DONE, &mut counter());
    thread.line(IDLE, &mut counter());
    thread
}

/// The answer of Claude Code to the control request with this ID.
fn answer(id: &serde_json::Value, subtype: &str) -> String {
    format!(
        r#"{{"type":"control_response","response":{{"subtype":"{subtype}","request_id":{id}}}}}"#
    )
}

#[test]
fn th110_claude_code_gets_the_change_at_once_and_the_row_says_it_when_claude_code_took_it() {
    let mut thread = held_claude();
    let steps = thread
        .set(Changes {
            model: Some(model("sonnet")),
            mode: Some(Mode::Ask),
            ..Changes::default()
        })
        .expect("changed");
    assert_eq!(
        subtypes(&steps),
        ["set_model", "set_permission_mode"],
        "also while a turn runs"
    );
    assert_eq!(writes(&steps)[1]["request"]["mode"], "manual");
    assert_eq!(
        thread.row.mode,
        Mode::FullAccess,
        "the program does not have it yet"
    );
    let ids: Vec<_> = writes(&steps)
        .iter()
        .map(|line| line["request_id"].clone())
        .collect();
    thread.line(&answer(&ids[1], "success"), &mut counter());
    assert_eq!(thread.row.mode, Mode::Ask);
    assert_eq!(thread.row.model, None);
    thread.line(&answer(&ids[0], "success"), &mut counter());
    assert_eq!(thread.row.model.as_deref(), Some("sonnet"));
}

#[test]
fn th110_a_mode_that_claude_code_refuses_is_never_shown_and_the_person_is_told() {
    let mut thread = idle_claude();
    let steps = thread
        .set(Changes {
            mode: Some(Mode::Ask),
            ..Changes::default()
        })
        .expect("changed");
    let said = thread.line(
        &answer(&writes(&steps)[0]["request_id"], "error"),
        &mut counter(),
    );
    assert_eq!(thread.row.mode, Mode::FullAccess);
    assert!(said.iter().any(|step| matches!(
        step,
        Step::Change(Change::Said { event: DriveEvent::Notice { text } }) if text.contains("mode")
    )));
}

#[test]
fn th110_claude_code_refuses_a_change_of_the_effort_with_words_and_nothing_changes() {
    let mut thread = idle_claude();
    let refused = thread.set(Changes {
        model: Some(model("opus")),
        effort: Some(Effort::High),
        ..Changes::default()
    });
    assert!(matches!(refused, Err(Refusal::CannotChange { said }) if said.contains("effort")));
    assert_eq!(
        thread.row.model, None,
        "a call with a refused part changes nothing"
    );
    assert_eq!(thread.row.effort, None);
}

#[test]
fn th110_codex_carries_the_change_on_each_later_turn_and_refuses_plan_mode() {
    let mut ids = counter();
    let (mut thread, _) = codex_in_a_turn(&mut ids);
    assert!(matches!(
        thread.set(Changes {
            mode: Some(Mode::Plan),
            ..Changes::default()
        }),
        Err(Refusal::CannotChange { .. })
    ));
    let steps = thread
        .set(Changes {
            model: Some(model("gpt-5.6-sol")),
            effort: Some(Effort::Medium),
            mode: Some(Mode::Ask),
        })
        .expect("changed");
    assert_eq!(writes(&steps).len(), 0, "nothing goes to the running turn");
    assert_eq!(
        thread.row.mode,
        Mode::FullAccess,
        "the running turn keeps its mode"
    );
    thread.line(
        r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
        &mut ids,
    );
    assert_eq!(
        thread.row.mode,
        Mode::Ask,
        "the change landed with the end of the turn"
    );
    assert_eq!(thread.row.effort, Some(Effort::Medium));
    let saved = thread.saved();
    assert_eq!(
        saved.codex_changed.mode,
        Some(Mode::Ask),
        "a restart keeps it"
    );
    for _ in 0..2 {
        let next = thread
            .send(&agent_threads::drive::Prompt::plain("go on"), true)
            .expect("running");
        let params = &serde_json::from_str::<serde_json::Value>(&next[0]).expect("JSON")["params"];
        assert_eq!(params["model"], "gpt-5.6-sol");
        assert_eq!(params["effort"], "medium");
        assert_eq!(params["approvalPolicy"], "untrusted");
        thread.line(
            r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
            &mut ids,
        );
    }
}

#[test]
fn th110_a_thread_that_does_not_run_changes_nothing() {
    let mut thread = claude();
    assert_eq!(
        thread.set(Changes {
            mode: Some(Mode::Ask),
            ..Changes::default()
        }),
        Err(Refusal::NotRunning)
    );
}

#[test]
fn th110_an_answer_of_claude_code_after_a_restart_of_the_runner_still_changes_the_row() {
    let mut thread = idle_claude();
    let steps = thread
        .set(Changes {
            mode: Some(Mode::Ask),
            ..Changes::default()
        })
        .expect("changed");
    let id = writes(&steps)[0]["request_id"].clone();
    let mut again = Thread::restore(thread.saved(), None);
    again.line(&answer(&id, "success"), &mut counter());
    assert_eq!(again.row.mode, Mode::Ask);
    let next = again
        .set(Changes {
            mode: Some(Mode::FullAccess),
            ..Changes::default()
        })
        .expect("changed");
    assert_ne!(
        writes(&next)[0]["request_id"],
        id,
        "an ID is never made two times"
    );
}

#[test]
fn th110_a_codex_change_waits_while_a_turn_start_is_not_answered() {
    let mut ids = counter();
    let (mut thread, _) = codex_in_a_turn(&mut ids);
    thread.line(
        r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
        &mut ids,
    );
    thread
        .send(&agent_threads::drive::Prompt::plain("next"), true)
        .expect("running");
    thread
        .set(Changes {
            mode: Some(Mode::Ask),
            ..Changes::default()
        })
        .expect("changed");
    assert_eq!(
        thread.row.mode,
        Mode::FullAccess,
        "the turn that starts has the old mode"
    );
}
