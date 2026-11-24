use agent_threads::control::{Action, Change, Outcome, RunState};
use agent_threads::drive::{Answer, Mode, Program};
use uuid::Uuid;

use super::super::gone;
use super::{Kit, change, child_pid, state_is};

/// Starts a thread, answers its request, and waits for its turn to end: the thread, the request
/// and the child that the made program left.
async fn start_and_answer(kit: &Kit) -> (Uuid, Uuid, u32) {
    let mut client = kit.client().await;
    let watch = client.call(Action::Watch).await;
    assert_eq!(client.done(watch).await, Outcome::Ok);
    let start = client
        .call(Action::Start {
            provider_address: None,
            program: Program::ClaudeCode,
            folder: kit.path("work"),
            mode: Mode::FullAccess,
            model: None,
            effort: None,
            speed: None,
            prompt: "Run the command".to_owned(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
            worktree: false,
            base: None,
        })
        .await;
    let Outcome::Started { thread, .. } = client.done(start).await else {
        panic!("the thread started");
    };
    let asked = client
        .until(|reply| matches!(change(reply), Some(Change::Asked { .. })))
        .await;
    let Some(Change::Asked { request, detail }) = change(&asked) else {
        unreachable!()
    };
    assert_eq!(detail.subject, "sleep 300");
    let request: Uuid = *request;
    client.until(state_is(RunState::NeedsYou)).await;
    let answer = client
        .call(Action::Answer {
            thread,
            request,
            answer: Answer::Allow,
        })
        .await;
    assert_eq!(client.done(answer).await, Outcome::Ok);
    client.until(state_is(RunState::Idle)).await;
    let child = child_pid(&kit.path("work")).await;

    (thread, request, child)
}

#[tokio::test]
async fn th11_a_thread_starts_asks_is_answered_survives_a_restart_and_stops_with_its_children() {
    let mut kit = Kit::new();
    let (thread, request, child) = start_and_answer(&kit).await;

    kit.end_runner();
    kit.start();
    let mut client = kit.client().await;
    let list = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(list).await else {
        panic!("threads")
    };
    assert_eq!(threads.len(), 1);
    assert_eq!(
        threads[0].state,
        RunState::Idle,
        "the program stayed in the holder"
    );
    assert_eq!(
        threads[0].session.as_deref(),
        Some(thread.to_string().as_str())
    );
    let again = client
        .call(Action::Answer {
            thread,
            request,
            answer: Answer::Allow,
        })
        .await;
    assert!(
        matches!(client.done(again).await, Outcome::Refused { .. }),
        "one answer for each request"
    );

    let stop = client.call(Action::Stop { thread }).await;
    assert_eq!(
        client.done(stop).await,
        Outcome::Stopped { all_ended: true }
    );
    assert!(
        gone(child).await,
        "the child in its own group ended with the program"
    );
    let log = std::fs::read_to_string(kit.threads().join("runner.log")).expect("a log");
    assert!(!log.contains("Run the command"), "the log holds no prompt");
    for kind in ["start", "answer", "stop", "refusal"] {
        assert!(
            log.contains(&format!(r#""kind":"{kind}""#)),
            "{kind} in {log}"
        );
    }
}
