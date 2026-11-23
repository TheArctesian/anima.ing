//! The removal of a worktree: the work is saved one time, also when the removal stops in the
//! middle, and a worktree that is not whole any more is still removed.

use agent_threads::git::{GitAction, GitOutcome};

use super::super::{Bench, PERSON, target};
use super::budget::own_of;

#[cfg(unix)]
#[tokio::test]
async fn a_removal_that_stops_in_the_middle_keeps_the_whole_work_at_the_next_try() {
    use std::os::unix::fs::PermissionsExt as _;

    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("work.txt"), "work\n").expect("a file");
    // What an agent can leave: a folder that cannot be emptied.
    let locked = folder.join("zz/locked");
    std::fs::create_dir_all(&locked).expect("a folder");
    std::fs::write(locked.join("f"), "x").expect("a file");
    let mode = |mode| std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(&locked, mode(0o555)).expect("a mode");
    let first = bench.call(PERSON, GitAction::Remove).await;
    assert!(super::super::refused(&first).is_some(), "{first:?}");
    // The work is saved as it was. Until the removal ends, nothing takes a snapshot of what
    // is left of the files, also not a page that asks for the status.
    if folder.join(".git").exists() {
        let status = format!("{:?}", bench.call(PERSON, GitAction::Status).await);
        assert!(status.contains("did not end"), "{status}");
        assert!(bench.engine.turn_ended(target(), 9).await.is_err());
    }
    // The person makes the folder removable and tries again: some files are gone already.
    // An edit after the removal stopped: it is saved apart, not lost and not over the work.
    let late = folder.join("late.txt");
    let edited = std::fs::write(&late, "late\n").is_ok() && folder.join(".git").exists();
    std::fs::set_permissions(&locked, mode(0o755)).expect("a mode");
    let second = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed { kept: Some(kept) } = second else {
        panic!("not removed: {second:?}");
    };
    assert!(!folder.exists());
    for (file, text) in [("work.txt", "work"), ("a.txt", "one"), ("zz/locked/f", "x")] {
        let saved = ["show", &format!("{kept}:{file}")];
        assert_eq!(bench.git_in(&bench.project, &saved).await, text, "{file}");
    }
    if edited {
        let rest = ["show", &format!("{kept}-rest:late.txt")];
        assert_eq!(bench.git_in(&bench.project, &rest).await, "late");
    }
    assert_eq!(bench.engine.disk()[0].worktrees, 0);
}

#[tokio::test]
async fn a_worktree_that_is_not_whole_is_removed_with_its_work_saved() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("work.txt"), "work\n").expect("a file");
    // A lock that a `git` left when it was ended does not stop the next snapshot.
    std::fs::write(own_of(&bench).join("index.lock"), "").expect("a lock");
    let status = bench.call(PERSON, GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    // The agent removes the file that makes the folder a worktree.
    std::fs::remove_file(folder.join(".git")).expect("the file");
    let status = bench.call(PERSON, GitAction::Status).await;
    assert!(super::super::refused(&status).is_some(), "{status:?}");
    // The removal needs no `.git` file: the files are saved and the worktree goes.
    let removed = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed { kept: Some(kept) } = removed else {
        panic!("not removed: {removed:?}");
    };
    let saved = ["show", &format!("{kept}:work.txt")];
    assert_eq!(bench.git_in(&bench.project, &saved).await, "work");
    assert!(!folder.exists());
    assert!(!own_of(&bench).exists(), "the folder of the runner stayed");
    // Nothing holds the project now.
    assert_eq!(bench.engine.disk()[0].worktrees, 0);
    bench
        .engine
        .forget_project(&bench.project)
        .expect("the copy of the project");
}

#[tokio::test]
async fn the_work_of_an_earlier_removal_stays_when_the_thread_is_removed_again() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("first.txt"), "first\n").expect("a file");
    let GitOutcome::Removed { kept: Some(kept) } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    let earlier = bench.git_in(&bench.project, &["rev-parse", &kept]).await;
    // The thread gets its worktree again, from its branch: the unsaved file is not in it.
    let folder = bench.worktree().await;
    assert!(!folder.join("first.txt").exists());
    std::fs::write(folder.join("second.txt"), "second\n").expect("a file");
    let again = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(again, GitOutcome::Removed { .. }), "{again:?}");
    let now = ["show", &format!("{kept}:second.txt")];
    assert_eq!(bench.git_in(&bench.project, &now).await, "second");
    let before = format!(
        "refs/anima/kept-before/{}/{earlier}:first.txt",
        super::super::THREAD
    );
    assert_eq!(
        bench.git_in(&bench.project, &["show", &before]).await,
        "first"
    );
}
