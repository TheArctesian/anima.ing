//! What an agent can plant in its worktree or in the Git folder of the repository, and that no
//! run of the runner follows it: a `.git` file or a `core.worktree` that points elsewhere, a
//! `HEAD` on a branch of the person, a filter with an odd name or in the configuration of the
//! worktree.

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};

use super::{Bench, PERSON, refused};

fn commit(new_branch: Option<&str>) -> GitAction {
    GitAction::Commit {
        message: "planted".to_owned(),
        reviewed: String::new(),
        files: None,
        new_branch: new_branch.map(str::to_owned),
    }
}

#[tokio::test]
async fn a_git_file_that_points_at_the_checkout_of_the_person_sends_no_run_there() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(bench.project.join("staged.txt"), "of the person\n").expect("a file");
    bench.git_in(&bench.project, &["add", "staged.txt"]).await;
    let before = bench.person_sees().await;
    let pointer = format!("gitdir: {}\n", bench.project.join(".git").display());
    std::fs::write(folder.join(".git"), pointer).expect("the planted file");
    std::fs::write(folder.join("a.txt"), "by the agent\n").expect("a file");
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    std::fs::write(folder.join("a.txt"), "by the agent\n").expect("a file");
    let committed = bench.call(PERSON, commit(Some("feature/planted"))).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    assert_eq!(
        bench.person_sees().await,
        before,
        "the checkout of the person changed"
    );
    assert_eq!(
        bench
            .git_in(&bench.project, &["log", "--format=%s", "main"])
            .await,
        "first"
    );
}

#[tokio::test]
async fn a_work_folder_set_in_the_configuration_is_not_cleaned_or_restored() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let victim = bench.home.parent().expect("the root").join("victim");
    std::fs::create_dir_all(victim.join("docs")).expect("a folder");
    std::fs::write(victim.join("docs/notes.txt"), "of the person\n").expect("a file");
    bench
        .git_in(&folder, &["config", "extensions.worktreeConfig", "true"])
        .await;
    let place = victim.to_string_lossy().into_owned();
    bench
        .git_in(&folder, &["config", "--worktree", "core.worktree", &place])
        .await;
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    bench.turn_ended(1).await;
    let text = std::fs::read_to_string(victim.join("docs/notes.txt")).expect("the file is there");
    assert_eq!(text, "of the person\n");
    assert!(
        !victim.join("a.txt").exists(),
        "a file was restored into the other folder"
    );
}

#[tokio::test]
async fn a_head_that_names_a_branch_of_the_person_is_not_committed_to_or_pushed() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    bench.git_in(&bench.project, &["branch", "release"]).await;
    let folder = bench.worktree().await;
    let git_dir = bench
        .git_in(&folder, &["rev-parse", "--absolute-git-dir"])
        .await;
    for branch in ["main", "release"] {
        let head = format!("ref: refs/heads/{branch}\n");
        std::fs::write(std::path::Path::new(&git_dir).join("HEAD"), head)
            .expect("the planted HEAD");
        std::fs::write(folder.join("a.txt"), "by the agent\n").expect("a file");
        let committed = bench.call(PERSON, commit(None)).await;
        assert_eq!(
            refused(&committed),
            Some(&GitRefusal::NotOurBranch),
            "{branch}"
        );
        for default_branch_allowed in [false, true] {
            let pushed = bench
                .call(
                    PERSON,
                    GitAction::Push {
                        default_branch_allowed,
                    },
                )
                .await;
            assert!(
                matches!(
                    refused(&pushed),
                    Some(GitRefusal::NotOurBranch | GitRefusal::DefaultBranch)
                ),
                "{branch}: {pushed:?}"
            );
        }
        let page = bench.call(PERSON, GitAction::PullRequest).await;
        assert!(refused(&page).is_some(), "{branch}: {page:?}");
        assert_eq!(
            bench
                .git_in(&bench.project, &["log", "--format=%s", branch])
                .await,
            "first"
        );
    }
    let origin = bench.project.with_file_name("origin.git");
    assert_eq!(
        bench
            .git_in(&origin, &["for-each-ref", "--format=%(refname)"])
            .await,
        "refs/heads/main"
    );
}

#[tokio::test]
async fn a_filter_with_an_odd_name_or_in_the_configuration_of_the_worktree_is_not_started() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let mark = bench.home.join("filter-ran");
    let clean = format!("touch '{}'; cat", mark.display());
    bench
        .git_in(&folder, &["config", "filter.a=b.clean", &clean])
        .await;
    bench
        .git_in(&folder, &["config", "extensions.worktreeConfig", "true"])
        .await;
    bench
        .git_in(&folder, &["config", "--worktree", "filter.w.clean", &clean])
        .await;
    std::fs::write(
        folder.join(".gitattributes"),
        "*.txt filter=a=b\n*.md filter=w\n",
    )
    .expect("a file");
    std::fs::write(folder.join("new.txt"), "new\n").expect("a file");
    std::fs::write(folder.join("new.md"), "new\n").expect("a file");
    bench.turn_ended(1).await;
    assert!(matches!(
        bench.call(PERSON, GitAction::Status).await,
        GitOutcome::Status(_)
    ));
    assert!(matches!(
        bench.call(PERSON, GitAction::Revert { turn: 0 }).await,
        GitOutcome::Reverted { .. }
    ));
    assert!(!mark.exists(), "a filter of the repository ran");
}

#[tokio::test]
async fn an_edit_that_the_index_is_told_to_skip_is_in_the_checkpoint_and_is_reverted() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    for mark in ["--assume-unchanged", "--skip-worktree"] {
        bench
            .git_in(&folder, &["update-index", mark, "a.txt"])
            .await;
    }
    std::fs::write(folder.join("a.txt"), "hidden edit\n").expect("a file");
    bench.turn_ended(1).await;
    let turn = super::checkpoint::turn_ref(super::THREAD, 1);
    let kept = bench
        .git_in(&folder, &["show", &format!("{turn}:a.txt")])
        .await;
    assert_eq!(kept, "hidden edit");
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    let text = std::fs::read_to_string(folder.join("a.txt")).expect("the file");
    assert_eq!(text, "one\n");
}
