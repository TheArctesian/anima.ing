//! The calls of the queue (TH109) through the socket, as a client writes them: each frame is
//! read and answered. A field of a call that the frame holds too (`id`, `call`) made the runner
//! close the connection before.

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::runner::{CLAUDE, Kit};

#[tokio::test]
async fn th109_each_call_of_the_queue_is_read_and_answered_through_the_socket() {
    let kit = Kit::with(CLAUDE);
    std::fs::create_dir_all(kit.path("work")).expect("a folder");
    let mut client = kit.client().await;
    let start = client
        .call(Action::Start {
            provider_address: None,
            program: Program::ClaudeCode,
            folder: kit.path("work"),
            mode: Mode::FullAccess,
            model: None,
            effort: None,
            speed: None,
            prompt: "first".to_owned(),
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
    // The made Claude Code waits for an answer, so a queued prompt waits too.
    let queue = client
        .call(Action::Queue {
            thread,
            text: "later".to_owned(),
        })
        .await;
    assert_eq!(client.done(queue).await, Outcome::Ok);
    let gone = Uuid::now_v7();
    for action in [
        Action::Requeue {
            thread,
            queued: gone,
            text: "new words".to_owned(),
        },
        Action::Unqueue {
            thread,
            queued: gone,
        },
        Action::SendQueued {
            thread,
            queued: gone,
        },
    ] {
        let call = client.call(action.clone()).await;
        assert_eq!(
            client.done(call).await,
            Outcome::Refused {
                why: Refusal::RequestGone
            },
            "{action:?} was read and answered"
        );
    }
}
