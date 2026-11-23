//! The disk that the checkpoints of one thread use (ruling of the operator): each status and
//! each checkpoint writes the changed files into the Git folder of the runner for the thread.
//! That folder has a size; what no checkpoint needs goes first, and a thread that is over
//! its size all the same is refused with words that say what to do.

use agent_threads::git::{GitAction, GitOutcome};

use super::super::{Bench, PERSON, THREAD, checkpoint, target};
use super::as_the_person;
use super::review::noise;
use crate::threads::git::Engine;
use crate::threads::git::place::Home;

/// An engine on the records of the bench where the checkpoints of one thread can use `room`
/// bytes.
fn engine_with_budget(bench: &Bench, room: u64) -> Engine {
    let records = bench.home.with_file_name("records");
    let home = Home::at(&bench.home, &records).expect("a home");
    let git = bench.git.clone().with_own_room(room);
    Engine::new(git, home).with_launcher(as_the_person(None))
}

/// A file of the worktree with `bytes` that are new each time.
fn rewritten(folder: &std::path::Path, round: u8, bytes: usize) {
    let mut large = noise(bytes);
    large[0] = round;
    std::fs::write(folder.join("large.bin"), large).expect("a file");
}

#[tokio::test]
async fn a_file_that_changes_between_calls_does_not_fill_the_disk_of_the_records() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let engine = engine_with_budget(&bench, 600 * 1024);
    for round in 0..12 {
        rewritten(&folder, round, 200 * 1024);
        let status = engine.call(PERSON, target(), GitAction::Status).await;
        assert!(
            matches!(status, GitOutcome::Status(_)),
            "{round}: {status:?}"
        );
    }
    let used = engine.disk()[0].largest_thread;
    assert!(
        used > 0 && used < 1024 * 1024,
        "the thread uses {used} bytes"
    );
    // The checkpoint of the start is whole after all of it.
    let start = checkpoint::turn_ref(THREAD, 0);
    assert_eq!(
        bench.git_own(&["show", &format!("{start}:a.txt")]).await,
        "one"
    );
}

#[tokio::test]
async fn a_thread_over_its_size_is_refused_until_old_checkpoints_are_removed() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let engine = engine_with_budget(&bench, 600 * 1024);
    let mut refused_at = None;
    for turn in 1..=8_u8 {
        rewritten(&folder, turn, 200 * 1024);
        if let Err(why) = engine.turn_ended(target(), u32::from(turn)).await {
            let said = format!("{why:?}");
            assert!(said.contains("Remove old checkpoints"), "{said}");
            refused_at = Some(u32::from(turn));
            break;
        }
    }
    let refused_at = refused_at.expect("the checkpoints never went over their size");
    // The person removes the checkpoints of the first turns.
    let forgot = GitAction::Forget { before: refused_at };
    let GitOutcome::Checkpoints { checkpoints } = engine.call(PERSON, target(), forgot).await
    else {
        panic!("the checkpoints were not removed");
    };
    assert_eq!(checkpoints.len(), 0);
    engine
        .turn_ended(target(), refused_at)
        .await
        .expect("a checkpoint after the old ones went");
    let text = std::fs::read(folder.join("large.bin")).expect("the file");
    assert_eq!(text.len(), 200 * 1024, "the files of the worktree stay");
}

#[tokio::test(flavor = "multi_thread")]
async fn making_the_folder_small_loses_nothing_of_a_checkpoint_taken_at_the_same_moment() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    // One engine, as the runner has one: its lock for the worktree is what keeps a snapshot
    // and the making small apart. The size is one that the snapshots go over in a few rounds.
    let engine = engine_with_budget(&bench, 600 * 1024);
    let status = || engine.call(PERSON, target(), GitAction::Status);
    for round in 1..=6_u8 {
        // A snapshot that no checkpoint keeps, then the files of the checkpoint.
        rewritten(&folder, round + 100, 64 * 1024);
        let before = status().await;
        assert!(
            matches!(before, GitOutcome::Status(_)),
            "{round}: {before:?}"
        );
        rewritten(&folder, round, 64 * 1024);
        // A checkpoint and three snapshots, all at one time.
        let turn = u32::from(round);
        let (taken, ..) = tokio::join!(
            engine.turn_ended(target(), turn),
            status(),
            status(),
            status()
        );
        taken.expect("a checkpoint");
    }
    let made_small = bench.home.with_file_name("records");
    let made_small = made_small
        .join(THREAD.to_string())
        .join("engine.git/anima-small-at");
    assert!(made_small.exists(), "the folder was never made small");
    // Each checkpoint has each of its files, and the folder has no missing object.
    let whole = ["fsck", "--connectivity-only", "--no-dangling"];
    bench.git_own(&whole).await;
    for turn in 0..=6_u8 {
        let name = checkpoint::turn_ref(THREAD, u32::from(turn));
        bench.git_own(&["ls-tree", "-r", &name]).await;
        bench
            .git_own(&["cat-file", "-e", &format!("{name}:a.txt")])
            .await;
    }
    let last = checkpoint::turn_ref(THREAD, 6);
    let size = ["cat-file", "-s", &format!("{last}:large.bin")];
    assert_eq!(bench.git_own(&size).await, (64 * 1024).to_string());
}
