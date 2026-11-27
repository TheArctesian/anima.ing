//! A worktree whose start was refused before an agent ran in it goes again with nothing saved;
//! one that was there already, and a branch that was there already, stay.

use agent_threads::git::{GitAction, GitOutcome};

use super::super::{Bench, PERSON, THREAD};
use crate::threads::git::worktree::Plan;

/// The worktrees of the project, as `git worktree list` of the person shows them.
async fn listed(bench: &Bench) -> usize {
    let list = bench
        .git_in(&bench.project, &["worktree", "list", "--porcelain"])
        .await;
    list.lines()
        .filter(|line| line.starts_with("worktree "))
        .count()
}

#[tokio::test]
async fn a_worktree_whose_start_is_refused_goes_again_with_nothing_saved() {
    let bench = Bench::new().await;
    let plan = Plan {
        thread: THREAD,
        project: &bench.project,
        base: None,
    };
    let pending = bench
        .engine
        .worktree_pending(plan, None)
        .await
        .expect("made");
    let made = pending.made().clone();
    assert!(!made.reused);
    let folder = made.worktree.folder().to_owned();
    let branch = bench.project.join(".git/refs/heads").join(&made.branch);
    assert!(folder.is_dir() && branch.is_file());
    assert_eq!(listed(&bench).await, 2);
    // Dropped where no runtime is, the undo runs in the drop.
    std::thread::spawn(move || drop(pending))
        .join()
        .expect("the drop");
    assert!(!folder.exists() && !branch.exists());
    assert!(!bench.home.join(THREAD.to_string()).exists());
    assert!(
        !bench.project.join(".git/refs/anima").exists(),
        "nothing is saved"
    );
    assert_eq!(listed(&bench).await, 1);
    // Dropped in the runtime, the undo runs on a blocking thread; a new make of the thread
    // waits for it and makes the worktree new.
    let pending = bench
        .engine
        .worktree_pending(plan, None)
        .await
        .expect("made");
    drop(pending);
    let again = bench
        .engine
        .worktree_pending(plan, None)
        .await
        .expect("made");
    assert!(!again.made().reused, "the dropped worktree was still there");
    let kept = again.keep();
    assert!(kept.worktree.folder().is_dir());
    assert!(!bench.project.join(".git/refs/anima").exists());
}

#[tokio::test]
async fn a_worktree_that_was_there_already_stays_when_its_start_is_refused() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let plan = Plan {
        thread: THREAD,
        project: &bench.project,
        base: None,
    };
    let pending = bench
        .engine
        .worktree_pending(plan, None)
        .await
        .expect("found");
    assert!(pending.made().reused);
    std::thread::spawn(move || drop(pending))
        .join()
        .expect("the drop");
    assert!(folder.is_dir());
    assert_eq!(listed(&bench).await, 2);
    let outcome = bench.call(PERSON, GitAction::Status).await;
    assert!(matches!(outcome, GitOutcome::Status(_)), "{outcome:?}");
}

#[tokio::test]
async fn an_action_of_the_person_waits_until_a_new_worktree_is_kept_or_gone() {
    let bench = Bench::new().await;
    let plan = Plan {
        thread: THREAD,
        project: &bench.project,
        base: None,
    };
    let pending = bench
        .engine
        .worktree_pending(plan, None)
        .await
        .expect("made");
    let removal = bench.call(PERSON, GitAction::Remove);
    tokio::pin!(removal);
    let early = tokio::time::timeout(std::time::Duration::from_millis(300), &mut removal).await;
    assert!(
        early.is_err(),
        "a removal ran on a worktree before its start: {early:?}"
    );
    drop(pending);
    let outcome = removal.await;
    assert!(
        !matches!(outcome, GitOutcome::Removed { .. }),
        "{outcome:?}"
    );
    assert!(
        !bench.project.join(".git/refs/anima").exists(),
        "nothing is saved"
    );
}

#[tokio::test]
async fn a_branch_that_was_there_before_stays_when_a_new_worktree_goes_again() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let outcome = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(outcome, GitOutcome::Removed { kept: Some(_), .. }),
        "{outcome:?}"
    );
    let plan = Plan {
        thread: THREAD,
        project: &bench.project,
        base: None,
    };
    let pending = bench
        .engine
        .worktree_pending(plan, None)
        .await
        .expect("made");
    assert!(!pending.made().reused);
    let branch = bench
        .project
        .join(".git/refs/heads")
        .join(&pending.made().branch);
    let folder = pending.made().worktree.folder().to_owned();
    std::thread::spawn(move || drop(pending))
        .join()
        .expect("the drop");
    assert!(!folder.exists());
    assert!(
        branch.is_file(),
        "the branch of the earlier worktree was taken"
    );
}
