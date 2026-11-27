//! Hold real worker results at the runner mailbox while authority changes on the sockets.

use agent_threads::control::{Outcome, Reply, RunState};
use agent_threads::git::read::{Outcome as Summary, Read};
use uuid::Uuid;

use super::super::{completed, socket};
use super::Completed;
use crate::threads::git::Target;
use crate::threads::wire::{self, Event, GitReadRoot};

mod actor;
mod fixture;
mod setup;
use actor::Actor;
use fixture::{Fixture, bounded, capable};

const THREAD: Uuid = Uuid::from_u128(0x1424_5678_9abc_def0_1234_5678_9abc_def0);
const REPOSITORY: &str = "work";

#[tokio::test]
async fn tc7_w3_boundary_drops_a_worker_reply_after_local_stop() {
    if !capable() {
        return;
    }
    let mut fixture = Fixture::open().await;
    let pending = fixture.pending(Read::Status).await;
    assert!(
        fixture
            .runner
            .command(wire::Action::Stop {
                thread: THREAD.to_string()
            })
            .await
            .is_some()
    );
    assert_no_reply(&mut fixture, pending).await;
}

#[tokio::test]
async fn tc7_w3_boundary_drops_a_worker_reply_while_the_holder_drains_reaped_output() {
    if !capable() {
        return;
    }
    let mut fixture = Fixture::open().await;
    let pending = fixture.pending(Read::Status).await;
    fixture.send(root_event(&fixture.peer.root, false)).await;
    fixture
        .send(Event::Lines {
            thread: THREAD.to_string(),
            first: 1,
            lines: vec![wire::Line {
                stream: wire::Stream::Err,
                text: "synthetic final output".to_owned(),
            }],
        })
        .await;
    assert_ne!(fixture.runner.threads[&THREAD].row.state, RunState::Stopped);
    assert_no_reply(&mut fixture, pending).await;
}

#[tokio::test]
async fn tc7_w3_boundary_drops_a_worker_reply_after_client_write_eof() {
    if !capable() {
        return;
    }
    let mut fixture = Fixture::open().await;
    let (client, pending) = fixture.pending(Read::Status).await;
    fixture.peer.prompt("half-close\n").await;
    assert_eq!(fixture.peer.line().await, "half-closed");
    bounded(async {
        while client.with_open(|| ()).is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert_no_reply(&mut fixture, (client, pending)).await;
}

#[tokio::test]
async fn tc7_w3_boundary_drops_an_old_holder_reply_and_accepts_the_survivor_on_reconnect() {
    if !capable() {
        return;
    }
    let mut fixture = Fixture::open().await;
    let pending = fixture.pending(Read::Status).await;
    fixture.disconnect_holder().await;
    assert_no_reply(&mut fixture, pending).await;
    // An old completion also stays invalid after a fresh Hello lists the same survivor.
    fixture.reconnect_holder().await;
    let pending = fixture.pending(Read::Status).await;
    fixture.disconnect_holder().await;
    fixture.reconnect_holder().await;
    assert_no_reply(&mut fixture, pending).await;
    let fresh = fixture.pending(Read::Status).await;
    assert!(matches!(
        deliver(&mut fixture, fresh).await.as_slice(),
        [Reply::Done {
            result: Outcome::GitRead {
                result: Summary::Status { .. }
            },
            ..
        }]
    ));
}

#[tokio::test]
async fn tc7_w3_boundary_drops_an_old_launch_reply_after_the_same_thread_relaunches() {
    if !capable() {
        return;
    }
    let mut fixture = Fixture::open().await;
    let pending = fixture.pending(Read::Status).await;
    let replacement = Actor::start(&fixture.path, 2);
    fixture.send(root_event(&fixture.peer.root, false)).await;
    fixture.send(root_event(&replacement.root, true)).await;
    assert_no_reply(&mut fixture, pending).await;
    fixture.peer = replacement;
    let fresh = fixture.pending(Read::Status).await;
    assert!(matches!(
        deliver(&mut fixture, fresh).await.as_slice(),
        [Reply::Done {
            result: Outcome::GitRead {
                result: Summary::Status { .. }
            },
            ..
        }]
    ));
}

#[tokio::test]
async fn tc7_w3_boundary_pages_real_engine_checkpoints_without_repeating_turns() {
    if !capable() {
        return;
    }
    let mut fixture = Fixture::open().await;
    for turn in [1, 3] {
        fixture
            .runner
            .git
            .engine()
            .expect("engine")
            .turn_ended(
                Target {
                    thread: THREAD,
                    repository: REPOSITORY,
                },
                turn,
            )
            .await
            .expect("a checkpoint");
    }
    let mut after = None;
    for (turn, cursor) in [(0, Some(0)), (1, Some(1)), (3, None)] {
        let pending = fixture.pending(Read::Checkpoints { limit: 1, after }).await;
        let replies = deliver(&mut fixture, pending).await;
        let [
            Reply::Done {
                result:
                    Outcome::GitRead {
                        result:
                            Summary::Checkpoints {
                                checkpoints,
                                next_cursor,
                            },
                    },
                ..
            },
        ] = replies.as_slice()
        else {
            panic!("expected one checkpoint page: {replies:?}")
        };
        assert_eq!(
            checkpoints
                .iter()
                .map(|checkpoint| checkpoint.turn)
                .collect::<Vec<_>>(),
            [turn]
        );
        assert_eq!(*next_cursor, cursor);
        after = *next_cursor;
    }
}

fn root_event(root: &GitReadRoot, enrolled: bool) -> Event {
    Event::GitReadRoot {
        root: root.clone(),
        enrolled,
    }
}

async fn assert_no_reply(fixture: &mut Fixture, pending: (socket::Client, Completed)) {
    let replies = deliver(fixture, pending).await;
    assert!(
        replies.is_empty(),
        "revoked worker enqueued a reply: {replies:?}"
    );
}

async fn deliver(
    fixture: &mut Fixture,
    (client, completion): (socket::Client, Completed),
) -> Vec<Reply> {
    fixture
        .runner
        .completed(completed::Completed::GitRead(Box::new(completion)));
    // The writer drains all queued replies before closing, so EOF is an enqueue barrier.
    client.close();
    drop(client);
    fixture.peer.replies().await
}
