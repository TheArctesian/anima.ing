//! Choosing a managed thread identity is unavailable on the ordinary user socket.
use super::Kit;
use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

#[tokio::test]
async fn local_socket_cannot_impersonate_managed_launch_owner() {
    let kit = Kit::new();
    let mut client = kit.client().await;
    let start = Action::Start {
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
    };
    let call = client
        .call(Action::StartManaged {
            thread: Uuid::from_u128(77),
            start: Box::new(start),
        })
        .await;
    assert_eq!(
        client.done(call).await,
        Outcome::Refused {
            why: Refusal::FromAnAgent
        }
    );
    let list = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(list).await else {
        panic!("thread inventory");
    };
    assert!(
        threads.is_empty(),
        "the refused launch must start no thread"
    );
}
