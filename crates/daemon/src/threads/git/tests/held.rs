//! How the engine is held to one worktree: an agent reads the thread it runs in and no other,
//! a folder put in the place of the worktree gets no run, and two tasks never change one
//! worktree at the same time.

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use super::trusted::{UNCOMMITTED, commit};
use super::{Bench, PERSON, THREAD, refused};
use crate::threads::git::Caller;

#[tokio::test]
async fn an_agent_reads_the_thread_it_runs_in_and_no_other() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let of_another = Caller {
        agent: true,
        thread: Some(Uuid::from_u128(7)),
        ..PERSON
    };
    let unbound = Caller {
        agent: true,
        ..PERSON
    };
    for caller in [of_another, unbound] {
        for action in [GitAction::Status, GitAction::Checkpoints, UNCOMMITTED] {
            let outcome = bench.call(caller, action).await;
            assert_eq!(refused(&outcome), Some(&GitRefusal::NoWorktree));
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_folder_put_in_the_place_of_the_worktree_gets_no_run() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    // What an agent can do: move its worktree away and put a copy of the pointer in a new
    // folder at the same path.
    let moved = folder.with_file_name("moved");
    std::fs::rename(&folder, &moved).expect("the move");
    std::fs::create_dir(&folder).expect("a new folder");
    std::fs::copy(moved.join(".git"), folder.join(".git")).expect("the pointer");
    std::fs::write(folder.join("planted.txt"), "planted\n").expect("a file");
    for action in [
        GitAction::Status,
        UNCOMMITTED,
        GitAction::Revert { turn: 0 },
    ] {
        let outcome = bench.call(PERSON, action).await;
        assert!(
            matches!(refused(&outcome), Some(GitRefusal::Failed { said }) if said.contains("replaced")),
            "{outcome:?}"
        );
    }
    assert!(
        folder.join("planted.txt").exists(),
        "a run changed the folder"
    );
}

#[tokio::test]
async fn a_checkpoint_and_a_commit_wait_while_the_worktree_is_held() {
    use std::time::{Duration, Instant};

    fn shared<T: Send + Sync + Clone + 'static>(_: &T) {}

    fn a_task<F: Future + Send>(_: &F) {}

    let bench = Bench::new().await;
    // The runner gives a clone of the engine to a task of its own for each call.
    shared(&bench.engine);
    a_task(&bench.call(PERSON, GitAction::Status));
    a_task(&bench.engine.turn_ended(super::target(), 1));
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
    // A different action of the person has the worktree, as a commit that waits for a key.
    let taken = bench.engine.alone(THREAD, "engine").await;
    let let_go = async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let at = Instant::now();
        drop(taken);
        at
    };
    let committed = async {
        let outcome = bench.call(PERSON, commit(&reviewed, None)).await;
        (outcome, Instant::now())
    };
    let checkpoint = async {
        bench.turn_ended(1).await;
        Instant::now()
    };
    // The commit asks first, so it is first when the worktree is free.
    let (free, (outcome, commit_done), checkpoint_done) =
        tokio::join!(let_go, committed, checkpoint);
    assert!(
        checkpoint_done >= free,
        "a checkpoint ran in a held worktree"
    );
    assert!(commit_done >= free, "a commit ran in a held worktree");
    // The checkpoint is of the branch after the commit.
    let head = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    let turn = super::checkpoint::turn_ref(THREAD, 1);
    let parent = bench.git_own(&["rev-parse", &format!("{turn}^")]).await;
    match outcome {
        GitOutcome::Committed { commit, .. } => {
            assert_eq!((head, parent), (commit.clone(), commit));
        }
        other => panic!("no commit: {other:?}"),
    }
}
