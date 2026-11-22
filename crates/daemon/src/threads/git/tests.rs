//! The Git of a thread on made repositories in temporary folders: no test speaks to a network,
//! and the only remote is a bare repository beside the made one.

mod person;
mod planted;
mod runs;
mod trusted;
mod turns;

use std::path::{Path, PathBuf};

use agent_threads::git::{GitAction, GitOutcome, GitRefusal, Scope};
use uuid::Uuid;

use super::place::Home;
use super::run::{Git, Run};
use super::{Caller, Engine, Target, checkpoint, worktree};

const THREAD: Uuid = Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);
const PERSON: Caller = Caller {
    agent: false,
    running: false,
    thread: None,
};

/// A made repository of a person with one commit on `main`, and the home of the worktrees.
struct Bench {
    _keep: tempfile::TempDir,
    git: Git,
    engine: Engine,
    project: PathBuf,
    home: PathBuf,
}

impl Bench {
    async fn new() -> Self {
        let keep = tempfile::tempdir().expect("a temporary folder");
        let root = std::fs::canonicalize(keep.path()).expect("its real path");
        let (project, home) = (root.join("engine"), root.join("worktrees"));
        let records = root.join("records");
        std::fs::create_dir_all(&project).expect("the project");
        std::fs::create_dir_all(&home).expect("the home");
        let git = Git::find(std::env::vars_os()).expect("git is installed");
        let bench = Self {
            engine: Engine::new(git.clone(), Home::at(&home, &records).expect("a home")),
            _keep: keep,
            git,
            project,
            home,
        };
        bench
            .git_in(
                &bench.project,
                &["init", "--quiet", "--initial-branch=main"],
            )
            .await;
        bench
            .git_in(&bench.project, &["config", "user.name", "Ada"])
            .await;
        bench
            .git_in(
                &bench.project,
                &["config", "user.email", "ada@example.invalid"],
            )
            .await;
        std::fs::write(bench.project.join(".gitignore"), "secret.env\n").expect("a file");
        std::fs::write(bench.project.join("a.txt"), "one\n").expect("a file");
        bench.git_in(&bench.project, &["add", "--all"]).await;
        bench
            .git_in(&bench.project, &["commit", "--quiet", "--message=first"])
            .await;
        bench
    }

    /// A command of the test itself, run as the person would in a terminal.
    async fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        self.git
            .text(Run::new(dir, args).for_the_person())
            .await
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"))
    }

    /// The worktree of the thread, made from the project.
    async fn worktree(&self) -> PathBuf {
        let plan = worktree::Plan {
            thread: THREAD,
            project: &self.project,
            base: None,
        };
        let made = self.engine.worktree(plan).await.expect("a worktree");
        made.worktree.folder().to_owned()
    }

    async fn call(&self, caller: Caller, mut action: GitAction) -> GitOutcome {
        let target = Target {
            thread: THREAD,
            repository: "engine",
        };
        // A commit with no snapshot named is of the files as the page would show them now.
        if let GitAction::Commit { reviewed, .. } = &mut action
            && reviewed.is_empty()
        {
            *reviewed = self.reviewed().await;
        }
        self.engine.call(caller, target, action).await
    }

    /// The snapshot of the uncommitted changes, as the page gets it before a commit.
    async fn reviewed(&self) -> String {
        let shown = GitAction::Changes {
            scope: Scope::Uncommitted,
            ignore_whitespace: false,
        };
        let target = Target {
            thread: THREAD,
            repository: "engine",
        };
        match self.engine.call(PERSON, target, shown).await {
            GitOutcome::Changes(changes) => changes.snapshot.unwrap_or_default(),
            _ => String::new(),
        }
    }

    async fn turn_ended(&self, turn: u32) {
        let target = Target {
            thread: THREAD,
            repository: "engine",
        };
        self.engine
            .turn_ended(target, turn)
            .await
            .expect("a checkpoint");
    }

    /// What the checkout of the person looks like: its branch, its commit and its status.
    async fn person_sees(&self) -> (String, String, String) {
        (
            self.git_in(&self.project, &["symbolic-ref", "--short", "HEAD"])
                .await,
            self.git_in(&self.project, &["rev-parse", "HEAD"]).await,
            self.git_in(&self.project, &["status", "--porcelain"]).await,
        )
    }
}

fn refused(outcome: &GitOutcome) -> Option<&GitRefusal> {
    match outcome {
        GitOutcome::Refused { why } => Some(why),
        _ => None,
    }
}

#[tokio::test]
async fn a_worktree_is_made_below_the_home_and_the_checkout_of_the_person_does_not_change() {
    let bench = Bench::new().await;
    let before = bench.person_sees().await;
    let folder = bench.worktree().await;
    assert!(folder.starts_with(&bench.home), "{}", folder.display());
    assert_eq!(folder, bench.home.join(THREAD.to_string()).join("engine"));
    let branch = bench
        .git_in(&folder, &["symbolic-ref", "--short", "HEAD"])
        .await;
    assert_eq!(branch, "anima/12345678");
    assert_eq!(bench.person_sees().await, before);
    assert_eq!(
        bench.worktree().await,
        folder,
        "the second call gives the same worktree"
    );
}

#[tokio::test]
async fn a_worktree_is_refused_for_a_folder_that_is_no_repository_and_for_a_taken_branch() {
    let bench = Bench::new().await;
    let plan = |project, base| worktree::Plan {
        thread: THREAD,
        project,
        base,
    };
    let bare_folder = bench.home.parent().expect("the root").join("plain");
    std::fs::create_dir_all(&bare_folder).expect("a folder");
    let no_repository = bench.engine.worktree(plan(&bare_folder, None)).await;
    assert_eq!(no_repository.err(), Some(GitRefusal::NotARepository));
    let unknown = bench
        .engine
        .worktree(plan(&bench.project, Some("no-such")))
        .await;
    assert_eq!(unknown.err(), Some(GitRefusal::UnknownBase));
    let option = bench
        .engine
        .worktree(plan(&bench.project, Some("--force")))
        .await;
    assert_eq!(option.err(), Some(GitRefusal::BadPath));
    bench
        .git_in(&bench.project, &["branch", "anima/12345678"])
        .await;
    let taken = bench.engine.worktree(plan(&bench.project, None)).await;
    assert_eq!(taken.err(), Some(GitRefusal::BranchExists));
    assert!(!bench.home.join(THREAD.to_string()).join("engine").exists());
}

#[tokio::test]
async fn a_checkpoint_takes_new_files_and_no_ignored_file_and_moves_no_branch() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let head = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    std::fs::write(folder.join("new.txt"), "new\n").expect("a file");
    std::fs::write(folder.join("secret.env"), "TOKEN=1\n").expect("a file");
    // A filter of the repository that would leave a mark if the runner started it (THR-050).
    let mark = bench.home.join("filter-ran");
    let clean = format!("touch '{}'; cat", mark.display());
    bench
        .git_in(&folder, &["config", "filter.mark.clean", &clean])
        .await;
    std::fs::write(folder.join(".gitattributes"), "*.txt filter=mark\n").expect("a file");
    bench.turn_ended(1).await;
    let listed = bench
        .git_in(
            &folder,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                &checkpoint::turn_ref(THREAD, 1),
            ],
        )
        .await;
    assert!(listed.contains("new.txt"), "{listed}");
    assert!(!listed.contains("secret.env"), "{listed}");
    assert!(matches!(
        bench.call(PERSON, GitAction::Status).await,
        GitOutcome::Status(_)
    ));
    assert!(matches!(
        bench.call(PERSON, GitAction::Revert { turn: 0 }).await,
        GitOutcome::Reverted { .. }
    ));
    bench.turn_ended(1).await;
    assert!(!mark.exists(), "a filter of the repository ran");
    assert_eq!(bench.git_in(&folder, &["rev-parse", "HEAD"]).await, head);
    assert_eq!(
        bench
            .git_in(&folder, &["diff", "--cached", "--name-only"])
            .await,
        ""
    );
    let GitOutcome::Checkpoints { checkpoints } = bench.call(PERSON, GitAction::Checkpoints).await
    else {
        panic!("no list");
    };
    let turns: Vec<u32> = checkpoints
        .iter()
        .map(|checkpoint| checkpoint.turn)
        .collect();
    assert_eq!(turns, [0, 1]);
}

#[tokio::test]
async fn a_removed_worktree_leaves_its_work_under_a_ref_and_its_branch_in_place() {
    let bench = Bench::new().await;
    let before = bench.person_sees().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("unsaved.txt"), "work\n").expect("a file");
    let GitOutcome::Removed { kept: Some(kept) } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    assert!(!folder.exists());
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
