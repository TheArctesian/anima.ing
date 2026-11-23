//! The profile that keeps the folder of Anima read-only in a thread of Codex (THR-041): it goes
//! only when Codex takes a turn whose policy leaves it, and a restart keeps what the record says.

use std::path::PathBuf;

use agent_threads::drive::{KeptFolder, Mode, Prompt};
use serde_json::{Value, json};

use crate::threads::runner::codex::{Changed, Codex};

fn kept() -> KeptFolder {
    KeptFolder::new(PathBuf::from("/home/you/anima-daemon"), uuid::Uuid::nil())
}

/// A thread of Codex in Ask in the home folder, its thread started (calls 1 and 2 answered).
fn in_ask() -> Codex {
    let mut codex =
        Codex::new(PathBuf::from("/home/you"), Mode::Ask, None, None).keeping(Some(kept()));
    codex.begin();
    codex.reply(1, true, &json!({})).expect("an answer");
    codex
        .reply(2, true, &json!({"thread": {"id": "th"}}))
        .expect("the thread");
    codex
}

/// The `id` and the policy of the next `turn/start` in `mode`.
fn turn(codex: &mut Codex, mode: Mode) -> (i64, Value) {
    codex.change(None, None, Some(mode));
    let line: Value =
        serde_json::from_str(&codex.prompt(&Prompt::plain("go")).remove(0)).expect("JSON");
    (
        line["id"].as_i64().expect("an id"),
        line["params"]["sandboxPolicy"]["type"].clone(),
    )
}

#[test]
fn thr_041_a_refused_turn_in_full_access_leaves_the_profile_in_force() {
    let mut codex = in_ask();
    let (full, policy) = turn(&mut codex, Mode::FullAccess);
    assert_eq!(policy, "dangerFullAccess");
    codex
        .reply(full, false, &json!({"message": "no"}))
        .expect("a refusal");
    assert!(codex.changed().keeps, "Codex did not take that turn");
    let (_, policy) = turn(&mut codex, Mode::Ask);
    assert_eq!(policy, Value::Null, "the profile still keeps the folder");
}

#[test]
fn thr_041_a_turn_sent_while_a_full_access_turn_waits_never_counts_on_the_profile() {
    let mut codex = in_ask();
    let (full, _) = turn(&mut codex, Mode::FullAccess);
    let (_, policy) = turn(&mut codex, Mode::Ask);
    assert_eq!(policy, "workspaceWrite", "the profile may be gone already");
    codex
        .reply(full, true, &json!({"turn": {"id": "u1"}}))
        .expect("taken");
    assert!(
        !codex.changed().keeps,
        "the policy took the place of the profile"
    );
}

#[test]
fn thr_041_a_restart_after_a_change_within_the_folder_sandbox_keeps_the_profile() {
    let mut codex = in_ask();
    let (edits, policy) = turn(&mut codex, Mode::AcceptEdits);
    assert_eq!(policy, Value::Null);
    codex
        .reply(edits, true, &json!({"turn": {"id": "u1"}}))
        .expect("taken");
    codex.turn_ended();
    let saved: Changed = codex.changed();
    assert!(saved.keeps);
    let mut again = Codex::new(PathBuf::from("/home/you"), Mode::Ask, None, None)
        .keeping(Some(kept()))
        .known(Some("th".to_owned()), None, saved);
    let (_, policy) = turn(&mut again, Mode::Ask);
    assert_eq!(policy, Value::Null, "the record says the profile holds");
}

#[test]
fn thr_041_a_thread_started_in_full_access_never_counts_on_a_profile_after_a_restart() {
    let full =
        Codex::new(PathBuf::from("/home/you"), Mode::FullAccess, None, None).keeping(Some(kept()));
    let saved = full.changed();
    assert!(!saved.keeps, "a start in Full access sends no profile");
    let mut again = Codex::new(PathBuf::from("/home/you"), Mode::Ask, None, None)
        .keeping(Some(kept()))
        .known(Some("th".to_owned()), None, saved);
    let (_, policy) = turn(&mut again, Mode::Ask);
    assert_eq!(policy, "workspaceWrite");
}

#[test]
fn thr_041_a_restart_while_a_full_access_turn_waits_never_counts_on_the_profile() {
    let mut codex = in_ask();
    let _ = turn(&mut codex, Mode::FullAccess);
    let saved = codex.changed();
    assert!(
        !saved.keeps,
        "Codex may take the turn while the runner is down"
    );
    let mut again = Codex::new(PathBuf::from("/home/you"), Mode::Ask, None, None)
        .keeping(Some(kept()))
        .known(Some("th".to_owned()), None, saved);
    let (_, policy) = turn(&mut again, Mode::Ask);
    assert_eq!(policy, "workspaceWrite");
}

#[test]
fn thr_041_a_leaving_turn_held_back_for_the_record_ends_and_the_profile_stays() {
    use agent_threads::control::{Change, ThreadRow};
    use agent_threads::drive::{DriveEvent, Program};

    use crate::threads::runner::thread::{Changes, Step, Thread};

    let codex = Codex::new(PathBuf::from("/home/you"), Mode::Ask, None, None).keeping(Some(kept()));
    let row = ThreadRow {
        folder: PathBuf::from("/home/you"),
        mode: Mode::Ask,
        ..super::row(Program::Codex)
    };
    let mut thread = Thread::new(row, None, Some(codex));
    let mut ids = super::counter();
    thread.begin("go");
    thread.line(r#"{"id":1,"result":{}}"#, &mut ids);
    thread.line(r#"{"id":2,"result":{"thread":{"id":"th"}}}"#, &mut ids);
    thread.line(r#"{"id":3,"result":{"turn":{"id":"u1"}}}"#, &mut ids);
    thread.line(
        r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
        &mut ids,
    );
    let to_full = Changes {
        mode: Some(Mode::FullAccess),
        ..Changes::default()
    };
    thread.set(to_full).expect("changed");
    let line = thread
        .send(&Prompt::plain("now"), true)
        .expect("running")
        .remove(0);
    assert!(thread.leaves_with(&line), "this turn leaves the profile");
    let steps = thread.held_back(&line);
    let told = steps.iter().any(|step| {
        matches!(step, Step::Change(Change::Said { event: DriveEvent::Notice { text } })
            if text.contains("could not write its record"))
    });
    assert!(told, "the person is told why the turn did not go");
    assert!(!thread.leaves_with(&line), "nothing waits for that line");
    let back_to_ask = Changes {
        mode: Some(Mode::Ask),
        ..Changes::default()
    };
    thread.set(back_to_ask).expect("changed");
    let next: Value = serde_json::from_str(
        &thread
            .send(&Prompt::plain("again"), true)
            .expect("running")
            .remove(0),
    )
    .expect("JSON");
    assert_eq!(
        next["params"]["sandboxPolicy"]["type"],
        Value::Null,
        "the profile still keeps the folder"
    );
}
