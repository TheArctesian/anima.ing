//! What an agent can leave in the repository for a later run (third security check): a hook
//! that the configuration names, a setting from before the first worktree, a remote that is a
//! folder, a merge left half done, and a yes of the person to a list that changed after.

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};

use super::trusted::{PUSH, UNCOMMITTED, commit};
use super::{Bench, PERSON, THREAD, refused};
use crate::threads::git::place::Home;
use crate::threads::git::run::Git;
use crate::threads::git::{Caller, Engine};

#[tokio::test]
async fn a_hook_that_the_configuration_names_runs_for_no_run_of_the_runner() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    // Git 2.54 runs such a hook for each ref that moves, each `add`, each checkout.
    for event in [
        "reference-transaction",
        "post-index-change",
        "post-checkout",
    ] {
        let key = format!("hook.evil-{event}.event");
        bench.git_in(&folder, &["config", &key, event]).await;
        let key = format!("hook.evil-{event}.command");
        let touch = format!("touch \"{}\"", ran.display());
        bench.git_in(&folder, &["config", &key, &touch]).await;
    }
    let _ = std::fs::remove_file(&ran);
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");

    assert!(matches!(
        bench.call(PERSON, UNCOMMITTED).await,
        GitOutcome::Changes(_)
    ));
    bench.turn_ended(1).await;
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    // The person accepts the new settings; their thread still runs no hook.
    assert_eq!(bench.trust(PERSON).await, GitOutcome::Trusted);
    std::fs::write(folder.join("a.txt"), "changed again\n").expect("a file");
    let committed = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    assert!(!ran.exists(), "a hook of the configuration ran");
}

#[tokio::test]
async fn what_was_in_the_repository_before_the_first_worktree_is_shown_before_a_commit() {
    let bench = Bench::new().await;
    // Left by an agent of an earlier thread, in the configuration that each worktree shares.
    bench
        .git_in(&bench.project, &["config", "credential.helper", "!evil"])
        .await;
    let plan = super::worktree::Plan {
        thread: THREAD,
        project: &bench.project,
        base: None,
    };
    let made = bench.engine.worktree(plan).await.expect("a worktree");
    std::fs::write(made.worktree.folder().join("a.txt"), "changed\n").expect("a file");
    let status = bench.status(PERSON).await;
    assert!(
        status
            .changed
            .iter()
            .any(|name| name == "credential.helper"),
        "{:?}",
        status.changed
    );
    let unaccepted = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(refused(&unaccepted), Some(GitRefusal::Changed { .. })),
        "{unaccepted:?}"
    );
}

#[tokio::test]
async fn a_yes_counts_only_for_the_list_that_was_shown_and_not_while_a_turn_runs() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    bench
        .git_in(&folder, &["config", "core.askPass", "/tmp/first"])
        .await;
    let shown = bench.status(PERSON).await.accept;
    // After the person read the list, one more setting.
    bench
        .git_in(&folder, &["config", "credential.helper", "!evil"])
        .await;
    let late = GitAction::Trust {
        shown: shown.clone(),
    };
    let outcome = bench.call(PERSON, late.clone()).await;
    assert!(
        matches!(refused(&outcome), Some(GitRefusal::Changed { what }) if what.len() == 2),
        "{outcome:?}"
    );
    let busy = Caller {
        running: true,
        ..PERSON
    };
    let shown = bench.status(PERSON).await.accept;
    let outcome = bench.call(busy, GitAction::Trust { shown }).await;
    assert_eq!(refused(&outcome), Some(&GitRefusal::Running));
    assert_eq!(
        bench.status(PERSON).await.changed,
        ["core.askpass", "credential.helper"]
    );
}

#[tokio::test]
async fn a_push_to_a_folder_of_this_computer_is_refused() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let committed = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    // The engine as the runner makes it: the tests alone allow a remote that is a folder.
    let root = bench.home.parent().expect("the root");
    let home = Home::at(&bench.home, &root.join("records")).expect("a home");
    let git = Git::find(std::env::vars_os()).expect("git is installed");
    let engine = Engine::new(git, home);
    let outcome = engine.call(PERSON, super::target(), PUSH).await;
    assert!(
        matches!(refused(&outcome), Some(GitRefusal::Failed { .. })),
        "{outcome:?}"
    );
    let origin = bench.project.with_file_name("origin.git");
    let branches = bench.git_in(&origin, &["branch", "--list"]).await;
    assert!(!branches.contains("anima/"), "the push reached the folder");
}

#[tokio::test]
async fn a_merge_left_half_done_gives_the_commit_of_the_person_no_second_parent() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    // An agent leaves the marks of a merge and of a cherry-pick in the Git folder.
    let who = [
        "-c",
        "user.name=Mallory",
        "-c",
        "user.email=m@example.invalid",
    ];
    let other = [&who[..], &["commit-tree", "HEAD^{tree}", "-m", "planted"]].concat();
    let other = bench.git_in(&folder, &other).await;
    let git_dir = bench
        .git_in(&folder, &["rev-parse", "--absolute-git-dir"])
        .await;
    for mark in ["MERGE_HEAD", "CHERRY_PICK_HEAD"] {
        let file = std::path::Path::new(&git_dir).join(mark);
        std::fs::write(file, format!("{other}\n")).expect("a mark");
    }
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let before = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    let committed = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    let seen = ["log", "-1", "--format=%P %an", "HEAD"];
    assert_eq!(bench.git_in(&folder, &seen).await, format!("{before} Ada"));
}
