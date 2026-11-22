//! The actions of the person (commit, push, pull request) and each refusal: an agent, a thread
//! with no worktree, a path that leaves the worktree, a push that was not allowed.

use agent_threads::git::{GitAction, GitOutcome, GitRefusal, Scope};

use super::{Bench, PERSON, THREAD, refused};
use crate::threads::git::Caller;

fn commit(message: &str, files: Option<&[&str]>, new_branch: Option<&str>) -> GitAction {
    GitAction::Commit {
        message: message.to_owned(),
        reviewed: String::new(),
        files: files.map(|files| files.iter().map(|&file| file.to_owned()).collect()),
        new_branch: new_branch.map(str::to_owned),
    }
}

const PUSH: GitAction = GitAction::Push {
    default_branch_allowed: false,
};

impl Bench {
    /// A bare repository beside the project as its `origin`, with `main` pushed to it.
    pub(super) async fn with_origin(&self) {
        let origin = self.project.with_file_name("origin.git");
        let place = origin.to_string_lossy().into_owned();
        self.git_in(
            &self.project,
            &["init", "--quiet", "--bare", "--initial-branch=main", &place],
        )
        .await;
        self.git_in(&self.project, &["remote", "add", "origin", &place])
            .await;
        self.git_in(&self.project, &["push", "--quiet", "origin", "main"])
            .await;
    }
}

#[tokio::test]
async fn an_agent_is_refused_each_action_that_changes_something_and_may_read() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let agent = Caller {
        agent: true,
        running: false,
        thread: Some(THREAD),
        ..PERSON
    };
    for action in [
        commit("by an agent", None, None),
        PUSH,
        GitAction::Push {
            default_branch_allowed: true,
        },
        GitAction::PullRequest,
        GitAction::Revert { turn: 0 },
        GitAction::Remove,
    ] {
        let outcome = bench.call(agent, action.clone()).await;
        assert_eq!(
            refused(&outcome),
            Some(&GitRefusal::FromAnAgent),
            "{action:?}"
        );
    }
    assert!(matches!(
        bench.call(agent, GitAction::Status).await,
        GitOutcome::Status(_)
    ));
    assert_eq!(
        bench.git_in(&folder, &["log", "--format=%s"]).await,
        "first"
    );
    assert!(folder.join("a.txt").exists());
}

#[tokio::test]
async fn a_thread_with_no_worktree_of_the_runner_is_refused_each_action() {
    let bench = Bench::new().await;
    let before = bench.person_sees().await;
    let none = bench.call(PERSON, commit("m", None, None)).await;
    assert_eq!(refused(&none), Some(&GitRefusal::NoWorktree));
    // A link at the place of the worktree that leads to the checkout of the person.
    let place = bench.home.join(THREAD.to_string());
    std::fs::create_dir_all(&place).expect("the folder of the thread");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&bench.project, place.join("engine")).expect("a link");
    std::fs::write(bench.project.join("a.txt"), "the work of the person\n").expect("a file");
    for action in [
        commit("m", None, None),
        GitAction::Revert { turn: 0 },
        GitAction::Remove,
        PUSH,
    ] {
        let outcome = bench.call(PERSON, action.clone()).await;
        assert!(
            matches!(
                refused(&outcome),
                Some(GitRefusal::BadPath | GitRefusal::NoWorktree)
            ),
            "{action:?}: {outcome:?}"
        );
    }
    let text = std::fs::read_to_string(bench.project.join("a.txt")).expect("the file");
    assert_eq!(text, "the work of the person\n");
    assert_eq!(bench.person_sees().await.1, before.1);
}

#[tokio::test]
async fn a_commit_takes_all_files_or_the_chosen_ones_and_refuses_a_path_that_leaves() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    std::fs::write(folder.join("b.txt"), "b\n").expect("a file");
    for path in [
        "../engine/a.txt",
        "/etc/hosts",
        ".git/config",
        "--all",
        ":(top)a.txt",
    ] {
        let outcome = bench.call(PERSON, commit("m", Some(&[path]), None)).await;
        assert_eq!(refused(&outcome), Some(&GitRefusal::BadPath), "{path}");
    }
    let empty = bench.call(PERSON, commit("  \n", None, None)).await;
    assert_eq!(refused(&empty), Some(&GitRefusal::EmptyMessage));
    let option = bench
        .call(PERSON, commit("m", None, Some("--orphan")))
        .await;
    assert_eq!(refused(&option), Some(&GitRefusal::BadPath));
    let chosen = bench
        .call(
            PERSON,
            commit("--amend is only a message", Some(&["b.txt"]), None),
        )
        .await;
    assert!(matches!(chosen, GitOutcome::Committed { .. }), "{chosen:?}");
    assert_eq!(
        bench
            .git_in(&folder, &["show", "--format=%s", "--name-only", "HEAD"])
            .await,
        "--amend is only a message\n\nb.txt"
    );
    let rest = bench
        .call(PERSON, commit("the rest", None, Some("feature/rest")))
        .await;
    let GitOutcome::Committed { branch, .. } = rest else {
        panic!("no commit: {rest:?}");
    };
    assert_eq!(branch.as_deref(), Some("feature/rest"));
    let nothing = bench.call(PERSON, commit("again", None, None)).await;
    assert_eq!(refused(&nothing), Some(&GitRefusal::NothingToCommit));
    // The `git` of the agent sees the branch that the commit made, with nothing left over.
    let on = ["symbolic-ref", "--short", "HEAD"];
    assert_eq!(bench.git_in(&folder, &on).await, "feature/rest");
    assert_eq!(bench.git_in(&folder, &["status", "--porcelain"]).await, "");
    assert_eq!(
        bench
            .git_in(&folder, &["log", "-1", "--format=%an <%ae>"])
            .await,
        "Ada <ada@example.invalid>"
    );
    assert_eq!(
        bench
            .git_in(&bench.project, &["log", "--format=%s", "main"])
            .await,
        "first"
    );
}

#[tokio::test]
async fn a_push_goes_to_the_remote_once_and_a_pull_request_needs_a_pushed_branch() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    assert!(matches!(
        bench.call(PERSON, commit("change", None, None)).await,
        GitOutcome::Committed { .. }
    ));
    let early = bench.call(PERSON, GitAction::PullRequest).await;
    assert_eq!(refused(&early), Some(&GitRefusal::NotPushed));
    let pushed = bench.call(PERSON, PUSH).await;
    let GitOutcome::Pushed {
        remote,
        branch,
        up_to_date,
        ..
    } = pushed
    else {
        panic!("no push: {pushed:?}");
    };
    assert_eq!(
        (remote.as_str(), branch.as_str(), up_to_date),
        ("origin", "anima/12345678", false)
    );
    let origin = bench.project.with_file_name("origin.git");
    assert_eq!(
        bench
            .git_in(&origin, &["log", "--format=%s", "anima/12345678"])
            .await,
        "change\nfirst"
    );
    bench.git_in(&folder, &["fetch", "--quiet", "origin"]).await;
    let again = bench.call(PERSON, PUSH).await;
    assert!(
        matches!(
            again,
            GitOutcome::Pushed {
                up_to_date: true,
                ..
            }
        ),
        "{again:?}"
    );
    // The made remote is a folder, not a forge: the checks pass and no page can be made.
    let page = bench.call(PERSON, GitAction::PullRequest).await;
    assert_eq!(refused(&page), Some(&GitRefusal::UnknownForge));
}

#[tokio::test]
async fn a_push_is_refused_with_no_remote_and_for_the_default_branch_without_a_yes() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let alone = bench.call(PERSON, PUSH).await;
    assert_eq!(refused(&alone), Some(&GitRefusal::NoRemote));
    bench.with_origin().await;
    // The person added the remote after the worktree was made, and accepts that change.
    assert_eq!(bench.trust(PERSON).await, GitOutcome::Trusted);
    // The checkout of the person leaves `main`, so that the worktree can be on it.
    bench
        .git_in(
            &bench.project,
            &["switch", "--quiet", "--create", "elsewhere"],
        )
        .await;
    bench.git_in(&folder, &["switch", "--quiet", "main"]).await;
    std::fs::write(folder.join("a.txt"), "on main\n").expect("a file");
    let on_default = bench.call(PERSON, commit("on main", None, None)).await;
    assert_eq!(refused(&on_default), Some(&GitRefusal::NotOurBranch));
    let unasked = bench.call(PERSON, PUSH).await;
    assert_eq!(refused(&unasked), Some(&GitRefusal::DefaultBranch));
    let origin = bench.project.with_file_name("origin.git");
    assert_eq!(
        bench.git_in(&origin, &["log", "--format=%s", "main"]).await,
        "first"
    );
    let scope = GitAction::Changes {
        scope: Scope::Uncommitted,
        ignore_whitespace: true,
    };
    assert!(matches!(
        bench.call(PERSON, scope).await,
        GitOutcome::Changes(_)
    ));
}
