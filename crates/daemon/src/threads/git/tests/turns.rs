//! The changes of a turn and the revert to a checkpoint.

use agent_threads::git::{ChangeKind, GitAction, GitOutcome, GitRefusal, Scope};

use super::{Bench, PERSON, refused};
use crate::threads::git::Caller;

#[tokio::test]
async fn the_changes_of_a_turn_of_the_thread_and_of_now_have_files_counts_and_patches() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "one\ntwo\n").expect("a file");
    bench.turn_ended(1).await;
    std::fs::write(folder.join("b.txt"), "b\n").expect("a file");
    bench.turn_ended(2).await;
    let changes = |scope| GitAction::Changes {
        scope,
        ignore_whitespace: false,
    };
    let GitOutcome::Changes(turn) = bench.call(PERSON, changes(Scope::Turn { turn: 2 })).await
    else {
        panic!("no changes of the turn");
    };
    assert_eq!(turn.files.len(), 1);
    assert_eq!(
        (turn.files[0].path.as_str(), turn.files[0].kind),
        ("b.txt", ChangeKind::Added)
    );
    assert!(
        turn.files[0]
            .patch
            .as_deref()
            .is_some_and(|patch| patch.contains("+b"))
    );
    let GitOutcome::Changes(thread) = bench.call(PERSON, changes(Scope::Thread)).await else {
        panic!("no changes of the thread");
    };
    assert_eq!(
        (thread.files.len(), thread.additions, thread.deletions),
        (2, 2, 0)
    );
    let GitOutcome::Changes(now) = bench.call(PERSON, changes(Scope::Uncommitted)).await else {
        panic!("no changes of now");
    };
    assert_eq!(
        now.files.len(),
        2,
        "a new file that is not staged is a change too"
    );
    let none = bench.call(PERSON, changes(Scope::Turn { turn: 9 })).await;
    assert_eq!(refused(&none), Some(&GitRefusal::UnknownCheckpoint));
}

#[tokio::test]
async fn a_revert_puts_the_files_back_and_keeps_what_was_there_for_an_undo() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "turn one\n").expect("a file");
    bench.turn_ended(1).await;
    std::fs::write(folder.join("a.txt"), "turn two\n").expect("a file");
    std::fs::write(folder.join("later.txt"), "later\n").expect("a file");
    std::fs::write(folder.join("secret.env"), "TOKEN=1\n").expect("a file");
    bench.turn_ended(2).await;
    let head = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    let busy = Caller {
        running: true,
        ..PERSON
    };
    let while_running = bench.call(busy, GitAction::Revert { turn: 1 }).await;
    assert_eq!(refused(&while_running), Some(&GitRefusal::Running));
    let GitOutcome::Reverted { undo, .. } = bench.call(PERSON, GitAction::Revert { turn: 1 }).await
    else {
        panic!("no revert");
    };
    assert_eq!(
        std::fs::read_to_string(folder.join("a.txt"))
            .ok()
            .as_deref(),
        Some("turn one\n")
    );
    assert!(!folder.join("later.txt").exists());
    assert!(
        folder.join("secret.env").exists(),
        "an ignored file is not removed"
    );
    assert_eq!(bench.git_in(&folder, &["rev-parse", "HEAD"]).await, head);
    let kept = bench
        .git_in(&folder, &["show", &format!("{undo}:later.txt")])
        .await;
    assert_eq!(kept, "later");
    let GitOutcome::Checkpoints { checkpoints } = bench.call(PERSON, GitAction::Checkpoints).await
    else {
        panic!("no list");
    };
    assert_eq!(checkpoints.len(), 2, "the checkpoint of turn 2 is gone");
    let unknown = bench.call(PERSON, GitAction::Revert { turn: 7 }).await;
    assert_eq!(refused(&unknown), Some(&GitRefusal::UnknownCheckpoint));
}
