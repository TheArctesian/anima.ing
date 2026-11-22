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

#[cfg(unix)]
#[tokio::test]
async fn a_checkpoint_waits_for_a_commit_that_runs_on_the_same_worktree() {
    use std::time::{Duration, Instant};

    fn shared<T: Send + Sync + Clone + 'static>(_: &T) {}

    fn a_task<F: Future + Send>(_: &F) {}

    let bench = Bench::new().await;
    // The runner gives a clone of the engine to a task of its own for each call.
    shared(&bench.engine);
    a_task(&bench.call(PERSON, GitAction::Status));
    a_task(&bench.engine.turn_ended(super::target(), 1));
    bench.hook("sleep 1");
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
    let allowed = Caller {
        repository_programs: true,
        ..PERSON
    };
    assert_eq!(bench.trust(allowed).await, GitOutcome::Trusted);
    let slow_commit = async {
        let outcome = bench.call(allowed, commit(&reviewed, None)).await;
        (outcome, Instant::now())
    };
    let checkpoint = async {
        // The commit has the worktree by now, and its hook sleeps.
        tokio::time::sleep(Duration::from_millis(300)).await;
        bench.turn_ended(1).await;
        Instant::now()
    };
    let ((committed, commit_done), checkpoint_done) = tokio::join!(slow_commit, checkpoint);
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    assert!(checkpoint_done >= commit_done, "the two ran at one time");
    // The checkpoint is of the branch after the commit.
    let head = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    let turn = super::checkpoint::turn_ref(THREAD, 1);
    let parent = format!("{turn}^");
    assert_eq!(bench.git_in(&folder, &["rev-parse", &parent]).await, head);
}
