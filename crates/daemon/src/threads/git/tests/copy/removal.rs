//! The removal of a worktree: the work is saved one time, also when the removal stops in the
//! middle, and a worktree that is not whole any more is still removed.

use agent_threads::git::{GitAction, GitOutcome};

use super::super::{Bench, PERSON};
use super::budget::own_of;

mod nested;

#[cfg(unix)]
#[tokio::test]
async fn retirement_preserves_late_writes_through_existing_handles() {
    use rustix::fs::{Mode, OFlags, open, openat};
    use std::io::Write as _;

    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let mut writer = std::fs::OpenOptions::new()
        .append(true)
        .open(folder.join("a.txt"))
        .expect("an editor has the existing file open");
    let directory = open(&folder, OFlags::DIRECTORY | OFlags::RDONLY, Mode::empty())
        .expect("an existing working directory");
    let outcome = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed {
        recovery: Some(recovery),
        ..
    } = outcome
    else {
        panic!("not retired: {outcome:?}");
    };
    writeln!(writer, "after the snapshot").expect("a late edit");
    let created = openat(
        &directory,
        "late.txt",
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
        Mode::from_raw_mode(0o600),
    )
    .expect("a late new file");
    std::fs::File::from(created)
        .write_all(b"new work")
        .expect("the file");
    assert!(!folder.exists());
    assert_eq!(
        std::fs::read_to_string(recovery.join("a.txt")).expect("original file"),
        "one\nafter the snapshot\n"
    );
    assert_eq!(
        std::fs::read_to_string(recovery.join("late.txt")).expect("late file"),
        "new work"
    );
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
    let GitOutcome::Removed {
        kept: Some(kept), ..
    } = removed
    else {
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
    let GitOutcome::Removed {
        kept: Some(kept), ..
    } = bench.call(PERSON, GitAction::Remove).await
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

#[tokio::test]
async fn what_a_thread_puts_at_the_names_of_the_saved_work_does_not_stop_a_removal() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("work.txt"), "work\n").expect("a file");
    // Each thread of the project can write the refs of the repository: a lock that stays, and
    // a folder where the ref of the saved work must be.
    let kept_at = bench.project.join(".git/refs/anima/kept");
    let name = super::super::THREAD.to_string();
    std::fs::create_dir_all(kept_at.join(&name).join("in the way")).expect("a folder");
    std::fs::write(kept_at.join(format!("{name}.lock")), "").expect("a lock");
    let removed = bench.call(PERSON, GitAction::Remove).await;
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
async fn retirement_preserves_ignored_worktree_notes_outside_git() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join(".gitignore"), "local-notes.txt\n").expect("ignore rules");
    std::fs::write(folder.join("local-notes.txt"), "unfinished notes\n").expect("an ignored note");
    let outcome = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed {
        kept: Some(kept),
        recovery: Some(recovery),
    } = outcome
    else {
        panic!("not retired: {outcome:?}");
    };
    assert_eq!(
        std::fs::read_to_string(recovery.join("local-notes.txt")).expect("note stays"),
        "unfinished notes\n"
    );
    let names = bench
        .git_in(&bench.project, &["ls-tree", "-r", "--name-only", &kept])
        .await;
    assert!(!names.contains("local-notes.txt"), "{names}");
}

#[tokio::test]
async fn a_removed_worktree_leaves_its_work_under_a_ref_and_its_branch_in_place() {
    let bench = Bench::new().await;
    let before = bench.person_sees().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("unsaved.txt"), "work\n").expect("a file");
    std::fs::write(folder.join("a.txt"), "staged only\n").expect("staged file");
    bench.git_in(&folder, &["add", "a.txt"]).await;
    std::fs::write(folder.join("a.txt"), "later working copy\n").expect("unstaged version");
    let GitOutcome::Removed {
        kept: Some(kept),
        recovery: Some(recovery),
    } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    assert!(!folder.exists());
    assert_eq!(
        bench.git_in(&recovery, &["show", ":a.txt"]).await,
        "staged only"
    );
    let registered = bench
        .git_in(&bench.project, &["worktree", "list", "--porcelain"])
        .await;
    assert!(
        registered.contains(&format!("worktree {}", recovery.display())),
        "{registered}"
    );
    assert_eq!(
        bench
            .git_in(&bench.project, &["show", &format!("{kept}:unsaved.txt")])
            .await,
        "work"
    );
    bench
        .git_in(
            &bench.project,
            &["show-ref", "--verify", "refs/heads/anima/12345678"],
        )
        .await;
    assert_eq!(bench.person_sees().await, before);
    assert_eq!(
        bench.worktree().await,
        folder,
        "the thread gets its branch again"
    );
}
