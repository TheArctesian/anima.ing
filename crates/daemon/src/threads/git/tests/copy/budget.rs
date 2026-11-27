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
        let GitOutcome::Status(status) = status else {
            panic!("{round}: {status:?}");
        };
        // Also when the folder was made small in the middle of the snapshot: the one new
        // file is the one change, and each other file is as the last commit has it.
        let changed: Vec<&str> = status.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(changed, ["large.bin"], "round {round}");
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
            let lower = said.to_lowercase();
            assert!(lower.contains("remove old checkpoints"), "{said}");
            assert!(!lower.contains("git ended"), "{said}");
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

/// The Git folder of the runner for the worktree of the thread of the bench.
pub(super) fn own_of(bench: &Bench) -> std::path::PathBuf {
    let records = bench.home.with_file_name("records");
    records.join(THREAD.to_string()).join("engine.git")
}

#[tokio::test]
async fn what_a_failed_snapshot_left_and_a_file_with_a_hole_do_not_lock_the_thread() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let engine = engine_with_budget(&bench, 600 * 1024);
    // What a snapshot that failed left behind: it is no checkpoint and counts for nothing.
    let staging = own_of(&bench).join("staging");
    std::fs::create_dir_all(&staging).expect("a folder");
    std::fs::write(staging.join("left.bin"), noise(4 * 1024 * 1024)).expect("a file");
    let status = engine.call(PERSON, target(), GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    // A file that says a large size and uses no disk is not copied to its end.
    let hole = std::fs::File::create(folder.join("hole.bin")).expect("a file");
    hole.set_len(64 * 1024 * 1024).expect("a size");
    let refusal = engine.call(PERSON, target(), GitAction::Status).await;
    let said = format!("{refusal:?}");
    assert!(said.contains("larger than what the checkpoints"), "{said}");
    assert!(!staging.exists(), "the copies of the snapshot stayed");
    assert!(engine.disk()[0].largest_thread < 1024 * 1024);
    // The worktree is removed all the same, and the work is kept.
    std::fs::remove_file(folder.join("hole.bin")).expect("the file");
    std::fs::write(folder.join("work.txt"), "work\n").expect("a file");
    let tiny = engine_with_budget(&bench, 1);
    let removed = tiny.call(PERSON, target(), GitAction::Remove).await;
    let GitOutcome::Removed {
        kept: Some(kept), ..
    } = removed
    else {
        panic!("not removed: {removed:?}");
    };
    let saved = ["show", &format!("{kept}:work.txt")];
    assert_eq!(bench.git_in(&bench.project, &saved).await, "work");
}

#[tokio::test]
async fn a_worktree_whose_last_commit_cannot_be_brought_is_removed_with_its_files_kept() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("work.txt"), "work\n").expect("a file");
    // The branch of the thread names a commit that the repository does not have.
    let branch = bench.project.join(".git/refs/heads/anima/12345678");
    std::fs::write(branch, format!("{}\n", "1".repeat(40))).expect("the ref");
    let status = bench.call(PERSON, GitAction::Status).await;
    assert!(super::super::refused(&status).is_some(), "{status:?}");
    let removed = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed {
        kept: Some(kept), ..
    } = removed
    else {
        panic!("not removed: {removed:?}");
    };
    assert!(!folder.exists());
    for (file, text) in [("work.txt", "work"), ("a.txt", "one")] {
        let saved = ["show", &format!("{kept}:{file}")];
        assert_eq!(bench.git_in(&bench.project, &saved).await, text);
    }
}

#[tokio::test]
async fn a_commit_of_the_person_needs_no_room_in_a_copy_that_is_full() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let records = bench.home.with_file_name("records");
    let home = Home::at(&bench.home, &records).expect("a home");
    let git = bench.git.clone().with_copy_room(64 * 1024);
    let engine = Engine::new(git, home).with_launcher(as_the_person(None));
    // The copy of the project is over its size.
    let projects = std::fs::read_dir(records.join("projects")).expect("the projects");
    let copy = projects
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|kind| kind == "git"));
    let junk = copy.expect("the copy").join("objects/kept.bin");
    std::fs::write(junk, noise(512 * 1024)).expect("a file");
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
    let commit = GitAction::Commit {
        message: "by the person".to_owned(),
        reviewed,
        files: None,
        new_branch: None,
    };
    let committed = engine.call(PERSON, target(), commit).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    // The next actions work: the runner itself brought the commit into the copy.
    let status = engine.call(PERSON, target(), GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    engine.turn_ended(target(), 1).await.expect("a checkpoint");
}

#[tokio::test]
async fn a_shallow_project_keeps_its_ends_when_the_folder_of_a_thread_is_made_small() {
    let bench = Bench::new().await;
    std::fs::write(bench.project.join("a.txt"), "two\n").expect("a file");
    let second = ["commit", "--quiet", "--all", "--message=second"];
    bench.git_in(&bench.project, &second).await;
    let root = bench.home.parent().expect("the root");
    let shallow = root.join("shallow");
    let from = format!("file://{}", bench.project.display());
    let place = shallow.to_string_lossy().into_owned();
    let clone = [
        "clone",
        "--quiet",
        "--depth=1",
        from.as_str(),
        place.as_str(),
    ];
    bench.git_in(root, &clone).await;
    let engine = engine_with_budget(&bench, 600 * 1024);
    let plan = crate::threads::git::worktree::Plan {
        thread: THREAD,
        project: &shallow,
        base: None,
    };
    let made = engine.worktree(plan).await.expect("a worktree");
    let folder = made.worktree.folder().to_owned();
    let of = crate::threads::git::Target {
        thread: THREAD,
        repository: "shallow",
    };
    let ends = made.worktree.record().own.join("shallow");
    let before = std::fs::read_to_string(&ends).expect("the ends of the shallow clone");
    // The person removes each checkpoint, which makes the folder small.
    let forgot = engine
        .call(PERSON, of, GitAction::Forget { before: 99 })
        .await;
    assert!(
        matches!(forgot, GitOutcome::Checkpoints { .. }),
        "{forgot:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&ends).ok().as_deref(),
        Some(before.as_str())
    );
    // Snapshots that go over the size, and so make the folder small, work after it.
    for round in 1..=6_u8 {
        rewritten(&folder, round, 200 * 1024);
        let status = engine.call(PERSON, of, GitAction::Status).await;
        assert!(
            matches!(status, GitOutcome::Status(_)),
            "{round}: {status:?}"
        );
    }
    engine.turn_ended(of, 1).await.expect("a checkpoint");
}
