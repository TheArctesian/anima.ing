//! Recovery of nested repositories and their working files.

use super::{Bench, GitAction, GitOutcome, PERSON, own_of};

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
    let author = [
        "-c",
        "user.name=Nested",
        "-c",
        "user.email=nested@example.invalid",
    ];
    bench.git_in(&inside, &["add", "work.txt"]).await;
    bench
        .git_in(
            &inside,
            &[&author[..], &["commit", "-qm", "nested history"]].concat(),
        )
        .await;
    std::fs::write(inside.join("work.txt"), "index only\n").expect("a staged version");
    bench.git_in(&inside, &["add", "work.txt"]).await;
    std::fs::write(inside.join("work.txt"), "inner work\n").expect("a later working copy");
    // A checkpoint leaves it alone, as before.
    bench.turn_ended(1).await;
    let GitOutcome::Removed {
        kept: Some(kept),
        recovery: Some(recovery),
        ..
    } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    assert!(!folder.exists());
    let nested = recovery.join("sub");
    assert_eq!(
        bench.git_in(&nested, &["log", "-1", "--format=%s"]).await,
        "nested history"
    );
    assert_eq!(
        bench.git_in(&nested, &["show", ":work.txt"]).await,
        "index only"
    );
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
    let GitOutcome::Removed {
        kept: Some(kept), ..
    } = removed
    else {
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
async fn retirement_keeps_ignored_nested_files_out_of_the_companion_ref() {
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
    let content = own_of(&bench).join("lfs/objects/e2/92");
    std::fs::create_dir_all(&content).expect("a folder");
    std::fs::write(
        content.join("e292f0def1cdcf4f6c105008b52515ac0ad72622d5fc230a02cbd2b7bcb5f4e2"),
        "the large file\n",
    )
    .expect("a file");
    let removal = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed {
        kept: Some(kept),
        recovery: Some(recovery),
        ..
    } = removal
    else {
        panic!("not retired: {removal:?}");
    };
    assert_eq!(
        std::fs::read_to_string(recovery.join("sub/build/out.o")).expect("retained ignored file"),
        "built\n"
    );
    let names = ["ls-tree", "-r", "--name-only", kept.as_str()];
    let names = bench.git_in(&bench.project, &names).await;
    assert!(names.contains("sub/src.c"), "{names}");
    assert!(
        !names.contains("out.o"),
        "what the repository ignores was saved: {names}"
    );
    let kept_content = bench.project.join(
        ".git/lfs/objects/e2/92/e292f0def1cdcf4f6c105008b52515ac0ad72622d5fc230a02cbd2b7bcb5f4e2",
    );
    let text = std::fs::read_to_string(kept_content).expect("the content of the large file");
    assert_eq!(text, "the large file\n");
}

#[tokio::test]
async fn tc7_removal_leaves_a_file_that_replaced_a_submodule() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let commit = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    bench
        .git_in(
            &folder,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{commit},sub"),
            ],
        )
        .await;
    bench
        .git_in(&folder, &["commit", "-qm", "record a submodule"])
        .await;
    let replacement = folder.join("sub");
    std::fs::write(&replacement, "unpreserved work\n").expect("a file in place of the submodule");
    let result = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed {
        recovery: Some(recovery),
        ..
    } = result
    else {
        panic!("not retired: {result:?}");
    };
    assert_eq!(
        std::fs::read_to_string(recovery.join("sub")).expect("the work stays"),
        "unpreserved work\n"
    );
}

#[tokio::test]
async fn tc7_absorbed_submodule_metadata_can_be_located_from_the_reported_original_path() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let add = [
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        bench.project.to_str().expect("UTF-8 path"),
        "module",
    ];
    bench.git_in(&folder, &add).await;
    bench.git_in(&folder, &["commit", "-qm", "submodule"]).await;
    let module = folder.join("module");
    let link = std::fs::read_to_string(module.join(".git")).expect("relative Git link");
    let relative = link
        .trim_end()
        .strip_prefix("gitdir: ")
        .expect("Git directory pointer");
    assert!(std::path::Path::new(relative).is_relative(), "{link}");
    let git_dir = module
        .join(relative)
        .canonicalize()
        .expect("original admin directory");
    std::fs::write(module.join("a.txt"), "staged only\n").expect("staged file");
    bench.git_in(&module, &["add", "a.txt"]).await;
    std::fs::write(module.join("a.txt"), "working version\n").expect("working file");

    let outcome = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Removed {
        original: Some(original),
        recovery: Some(recovery),
        ..
    } = outcome
    else {
        panic!("expected a retirement receipt: {outcome:?}");
    };
    assert_eq!(original, folder);
    let origin = recovery
        .parent()
        .expect("recovery container")
        .join("original.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(origin).expect("durable origin"))
            .expect("origin record");
    assert_eq!(
        manifest["original_worktree"],
        folder.to_str().expect("path")
    );
    assert_eq!(
        std::fs::read_to_string(recovery.join("module/.git")).expect("retained pointer"),
        link
    );
    let spec = format!("--git-dir={}", git_dir.display());
    let work = format!("--work-tree={}", recovery.join("module").display());
    assert_eq!(
        bench
            .git_in(&bench.project, &[&spec, &work, "show", ":a.txt"])
            .await,
        "staged only"
    );
}
