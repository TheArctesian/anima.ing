//! What an agent can leave for a later run (second security review): a nested repository that
//! a `.gitmodules` tells git to look into, hooks and settings that changed since the person
//! accepted them, and an edit after the person looked at the changes.

use agent_threads::git::{GitAction, GitOutcome, GitRefusal, Scope};

use super::{Bench, PERSON, THREAD, refused};
use crate::threads::git::Caller;

pub(super) const PUSH: GitAction = GitAction::Push {
    default_branch_allowed: false,
};

pub(super) fn commit(reviewed: &str, files: Option<&[&str]>) -> GitAction {
    GitAction::Commit {
        message: "by the person".to_owned(),
        reviewed: reviewed.to_owned(),
        files: files.map(|files| files.iter().map(|&file| file.to_owned()).collect()),
        new_branch: None,
    }
}

pub(super) const UNCOMMITTED: GitAction = GitAction::Changes {
    scope: Scope::Uncommitted,
    ignore_whitespace: false,
};

/// The hooks that git runs for the commands of the engine.
pub(super) const HOOKS: [&str; 6] = [
    "reference-transaction",
    "post-index-change",
    "post-checkout",
    "pre-commit",
    "pre-push",
    "post-commit",
];

impl Bench {
    /// Writes each hook of the project that a commit, a checkout or a push could run; each
    /// worktree shares them.
    pub(super) fn hook(&self, body: &str) {
        let hooks = self.project.join(".git/hooks");
        std::fs::create_dir_all(&hooks).expect("the hooks");
        for name in HOOKS {
            let file = hooks.join(name);
            std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).expect("a hook");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let runs = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&file, runs).expect("a mode");
            }
        }
    }
}

#[tokio::test]
async fn a_nested_repository_that_gitmodules_opens_runs_no_program_of_its_own() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    // What an agent can do in its worktree: a repository inside it with a filter of its own,
    // named by the last commit of the branch, and a `.gitmodules` that says "look into it".
    let nested = folder.join("sub");
    std::fs::create_dir_all(&nested).expect("a folder");
    bench
        .git_in(&nested, &["init", "--quiet", "--initial-branch=main"])
        .await;
    std::fs::write(nested.join("f.txt"), "x\n").expect("a file");
    std::fs::write(nested.join(".gitattributes"), "f.txt filter=evil\n").expect("a file");
    let who = [
        "-c",
        "user.name=Agent",
        "-c",
        "user.email=agent@example.invalid",
    ];
    bench.git_in(&nested, &["add", "--all"]).await;
    let first = [&who[..], &["commit", "--quiet", "--message=nested"]].concat();
    bench.git_in(&nested, &first).await;
    let touch = format!("sh -c 'touch \"{}\"; cat'", ran.display());
    bench
        .git_in(&nested, &["config", "filter.evil.clean", &touch])
        .await;
    let modules = "[submodule \"sub\"]\n\tpath = sub\n\turl = ./sub\n\tignore = none\n";
    std::fs::write(folder.join(".gitmodules"), modules).expect("a file");
    bench.git_in(&folder, &["add", "--all"]).await;
    let link = [&who[..], &["commit", "--quiet", "--message=link"]].concat();
    bench.git_in(&folder, &link).await;
    // The same size as before, so that git must read the file to know that it changed.
    std::fs::write(nested.join("f.txt"), "z\n").expect("a file");
    let _ = std::fs::remove_file(&ran);

    let agent = Caller {
        agent: true,
        thread: Some(THREAD),
        ..PERSON
    };
    assert!(matches!(
        bench.call(agent, GitAction::Status).await,
        GitOutcome::Status(_)
    ));
    assert!(matches!(
        bench.call(agent, UNCOMMITTED).await,
        GitOutcome::Changes(_)
    ));
    bench.turn_ended(1).await;
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 1 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    assert!(!ran.exists(), "a program of the nested repository ran");
}

#[tokio::test]
async fn no_hook_of_the_repository_runs_and_a_changed_remote_waits_for_the_person() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let ran = folder.with_file_name("RAN");
    bench.hook(&format!("touch \"{}\"", ran.display()));
    // What an agent wrote: a helper for credentials, and a different place for a push.
    let helper = format!("!touch \"{}\"", ran.display());
    bench
        .git_in(&folder, &["config", "credential.helper", &helper])
        .await;
    let origin = bench.project.with_file_name("origin.git");
    let moved = bench.project.with_file_name("moved.git");
    std::fs::rename(&origin, &moved).expect("the move");
    let place = moved.to_string_lossy().into_owned();
    bench
        .git_in(&folder, &["remote", "set-url", "origin", &place])
        .await;
    let status = bench.status(PERSON).await;
    assert_eq!(status.changed.len(), 2, "the old place and the new one");
    for action in [commit("", None), PUSH] {
        let outcome = bench.call(PERSON, action).await;
        assert!(
            matches!(refused(&outcome), Some(GitRefusal::Changed { what }) if what.len() == 2),
            "{outcome:?}"
        );
    }
    let agent = Caller {
        agent: true,
        thread: Some(THREAD),
        ..PERSON
    };
    let by_agent = bench.trust(agent).await;
    assert_eq!(refused(&by_agent), Some(&GitRefusal::FromAnAgent));
    assert_eq!(bench.trust(PERSON).await, GitOutcome::Trusted);
    assert_eq!(bench.status(PERSON).await.changed, Vec::<String>::new());
    let committed = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    let pushed = bench.call(PERSON, PUSH).await;
    assert!(matches!(pushed, GitOutcome::Pushed { .. }), "{pushed:?}");
    let sent = ["log", "-1", "--format=%s", "anima/12345678"];
    assert_eq!(bench.git_in(&moved, &sent).await, "by the person");
    assert!(!ran.exists(), "a hook or a helper of the repository ran");
}

#[tokio::test]
async fn a_commit_holds_the_files_that_the_person_saw_and_no_later_edit() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "seen\n").expect("a file");
    std::fs::write(folder.join("new.txt"), "seen too\n").expect("a file");
    let GitOutcome::Changes(shown) = bench.call(PERSON, UNCOMMITTED).await else {
        panic!("no changes");
    };
    let reviewed = shown.snapshot.expect("a snapshot");
    // After the person looked: an edit and a new file.
    std::fs::write(folder.join("a.txt"), "not seen\n").expect("a file");
    std::fs::write(folder.join("backdoor.txt"), "not seen\n").expect("a file");

    let busy = Caller {
        running: true,
        ..PERSON
    };
    for action in [commit(&reviewed, None), PUSH] {
        let outcome = bench.call(busy, action).await;
        assert_eq!(refused(&outcome), Some(&GitRefusal::Running));
    }
    for unknown in ["HEAD", "--upload-pack=x", &"0".repeat(40)] {
        let outcome = bench.call(PERSON, commit(unknown, None)).await;
        let why = refused(&outcome);
        assert!(
            why == Some(&GitRefusal::NotReviewed),
            "{unknown}: {outcome:?}"
        );
    }
    let committed = bench
        .call(PERSON, commit(&reviewed, Some(&["a.txt"])))
        .await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    assert_eq!(bench.git_in(&folder, &["show", "HEAD:a.txt"]).await, "seen");
    let names = ["show", "--format=", "--name-only", "HEAD"];
    assert_eq!(bench.git_in(&folder, &names).await, "a.txt");
    // The branch moved, so the snapshot is not of the branch as it is now.
    let stale = bench.call(PERSON, commit(&reviewed, None)).await;
    assert_eq!(refused(&stale), Some(&GitRefusal::NotReviewed));
    assert_eq!(
        std::fs::read_to_string(folder.join("a.txt"))
            .ok()
            .as_deref(),
        Some("not seen\n"),
        "the files of the worktree are not changed by a commit"
    );
}

#[tokio::test]
async fn full_access_runs_no_hook_when_the_runner_gave_no_way_to_start_one() {
    let bench = Bench::new().await;
    let full = Caller {
        access: crate::threads::git::run::hooks::Access::Full,
        ..PERSON
    };
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    bench.hook(&format!("touch \"{}\"; exit 1", ran.display()));
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
    // The same records, and an engine that got no launcher from the runner.
    let records = bench.home.with_file_name("records");
    let home = crate::threads::git::place::Home::at(&bench.home, &records).expect("a home");
    let engine = crate::threads::git::Engine::new(bench.git.clone(), home);
    let committed = engine
        .call(full, super::target(), commit(&reviewed, None))
        .await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    assert!(!ran.exists(), "a hook of the repository ran");
}
