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
        let listed = [
            "for-each-ref",
            "--format=%(refname)",
            "refs/anima/kept-rest",
        ];
        let rest = bench.git_in(&bench.project, &listed).await;
        assert_eq!(rest.lines().count(), 1, "{rest}");
        let late = ["show", &format!("{rest}:late.txt")];
        assert_eq!(bench.git_in(&bench.project, &late).await, "late");
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

#[tokio::test]
async fn a_repository_inside_the_worktree_is_saved_before_the_worktree_goes() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    // What an agent does in normal work: a clone or a `git init` in a folder of its worktree.
    let inside = folder.join("sub");
    std::fs::create_dir_all(inside.join("deep")).expect("a folder");
    let init = ["init", "--quiet", "--initial-branch=main"];
    bench.git_in(&inside, &init).await;
    std::fs::write(inside.join("work.txt"), "inner work\n").expect("a file");
    std::fs::write(inside.join("deep/more.txt"), "more\n").expect("a file");
    // A checkpoint leaves it alone, as before.
    bench.turn_ended(1).await;
    let GitOutcome::Removed { kept: Some(kept) } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    assert!(!folder.exists());
    for (file, text) in [
        ("sub/work.txt", "inner work"),
        ("sub/deep/more.txt", "more"),
    ] {
        let saved = ["show", &format!("{kept}:{file}")];
        assert_eq!(bench.git_in(&bench.project, &saved).await, text, "{file}");
    }
    let names = ["ls-tree", "-r", "--name-only", kept.as_str()];
    let names = bench.git_in(&bench.project, &names).await;
    assert!(!names.contains(".git/"), "a Git folder was saved: {names}");
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
    let GitOutcome::Removed { kept: Some(kept) } = removed else {
        panic!("not removed: {removed:?}");
    };
    let saved = ["show", &format!("{kept}:work.txt")];
    assert_eq!(bench.git_in(&bench.project, &saved).await, "work");
}

#[cfg(unix)]
#[tokio::test]
async fn links_beside_a_named_repository_send_no_saved_file_out_of_the_runner_s_folder() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let outside = bench.home.with_file_name("outside");
    std::fs::create_dir_all(&outside).expect("a folder");
    std::fs::write(outside.join("victim"), "VICTIM\n").expect("a file");
    // The last commit of the branch names a repository at `sub`, and `sub` holds files.
    let id = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    let entry = format!("160000,{id},sub");
    let add_entry = ["update-index", "--add", "--cacheinfo", entry.as_str()];
    bench.git_in(&folder, &add_entry).await;
    let who = [
        "-c",
        "user.name=Agent",
        "-c",
        "user.email=a@example.invalid",
    ];
    let made = [&who[..], &["commit", "--quiet", "--message=named"]].concat();
    bench.git_in(&folder, &made).await;
    std::fs::create_dir_all(folder.join("sub/deep")).expect("a folder");
    std::fs::write(folder.join("sub/a"), "AGENT\n").expect("a file");
    std::fs::write(folder.join("sub/deep/new"), "AGENT\n").expect("a file");
    // Links where a guessed name for the saved files would be.
    std::fs::create_dir_all(folder.join("sub.files")).expect("a folder");
    let link = |to: &str, at: &str| {
        std::os::unix::fs::symlink(outside.join(to), folder.join(at)).expect("a link");
    };
    link("victim", "sub.files/a");
    link("", "sub.files/deep");
    let removed = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed { kept: Some(kept) } = removed else {
        panic!("not removed: {removed:?}");
    };
    let victim = std::fs::read_to_string(outside.join("victim")).expect("the file");
    assert_eq!(
        victim, "VICTIM\n",
        "a saved file was written through a link"
    );
    assert!(!outside.join("new").exists(), "a file was made outside");
    // The files of the named repository are saved, below a name that nothing could plant.
    let names = ["ls-tree", "-r", "--name-only", kept.as_str()];
    let names = bench.git_in(&bench.project, &names).await;
    let saved: Vec<&str> = names
        .lines()
        .filter(|name| name.starts_with("anima-kept-"))
        .collect();
    assert_eq!(saved.len(), 2, "{names}");
    assert!(
        saved.iter().any(|name| name.ends_with("/sub/deep/new")),
        "{names}"
    );
}

#[tokio::test]
async fn what_a_repository_inside_ignores_is_not_saved_and_large_file_contents_are() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let inside = folder.join("sub");
    std::fs::create_dir_all(inside.join("build")).expect("a folder");
    let init = ["init", "--quiet", "--initial-branch=main"];
    bench.git_in(&inside, &init).await;
    std::fs::write(inside.join(".gitignore"), "build/\n").expect("a file");
    std::fs::write(inside.join("build/out.o"), "built\n").expect("a file");
    std::fs::write(inside.join("src.c"), "source\n").expect("a file");
    // What Git LFS keeps beside the Git folder of the runner for a file of a snapshot.
    let content = own_of(&bench).join("lfs/objects/ab/cd");
    std::fs::create_dir_all(&content).expect("a folder");
    std::fs::write(content.join("abcd1234"), "the large file\n").expect("a file");
    let GitOutcome::Removed { kept: Some(kept) } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    let names = ["ls-tree", "-r", "--name-only", kept.as_str()];
    let names = bench.git_in(&bench.project, &names).await;
    assert!(names.contains("sub/src.c"), "{names}");
    assert!(
        !names.contains("out.o"),
        "what the repository ignores was saved: {names}"
    );
    let kept_content = bench.project.join(".git/lfs/objects/ab/cd/abcd1234");
    let text = std::fs::read_to_string(kept_content).expect("the content of the large file");
    assert_eq!(text, "the large file\n");
}
