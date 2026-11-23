//! What a snapshot stages: a file below a folder that became a link, a file that cannot be
//! read, and a large-file content that an earlier copy left in part.

use agent_threads::git::{GitAction, GitOutcome};

use super::super::{Bench, PERSON, refused};
use super::budget::own_of;

#[cfg(unix)]
#[tokio::test]
async fn a_committed_folder_that_became_a_link_does_not_stop_a_snapshot() {
    let bench = Bench::new().await;
    std::fs::create_dir_all(bench.project.join("d")).expect("a folder");
    std::fs::write(bench.project.join("d/f"), "in the folder\n").expect("a file");
    bench.git_in(&bench.project, &["add", "d/f"]).await;
    let commit = ["commit", "--quiet", "--message=a folder"];
    bench.git_in(&bench.project, &commit).await;
    let folder = bench.worktree().await;
    std::fs::create_dir_all(folder.join("else")).expect("a folder");
    std::fs::write(folder.join("else/f"), "elsewhere\n").expect("a file");
    std::fs::remove_dir_all(folder.join("d")).expect("the folder");
    std::os::unix::fs::symlink("else", folder.join("d")).expect("a link");
    let GitOutcome::Removed { kept: Some(kept) } = bench.call(PERSON, GitAction::Remove).await
    else {
        panic!("not removed");
    };
    let listed = ["ls-tree", "-r", kept.as_str()];
    let listed = bench.git_in(&bench.project, &listed).await;
    assert!(!listed.contains("d/f"), "{listed}");
    assert!(listed.contains("else/f"), "{listed}");
    let link = listed.lines().find(|line| line.ends_with("\td"));
    assert!(
        link.is_some_and(|line| line.starts_with("120000")),
        "{listed}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_that_cannot_be_read_is_named_with_what_to_do() {
    use std::os::unix::fs::PermissionsExt as _;

    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let closed = folder.join("closed.txt");
    std::fs::write(&closed, "x\n").expect("a file");
    std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000)).expect("a mode");
    let first = bench.call(PERSON, GitAction::Remove).await;
    assert!(refused(&first).is_some(), "{first:?}");
    let said = format!("{first:?}");
    assert!(said.contains("closed.txt"), "{said}");
    assert!(said.contains("make it readable or delete it"), "{said}");
    std::fs::remove_file(&closed).expect("the file");
    let second = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(second, GitOutcome::Removed { .. }), "{second:?}");
}

#[tokio::test]
async fn a_large_file_content_that_was_copied_in_part_is_copied_whole() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let content = own_of(&bench).join("lfs/objects/ab/cd");
    std::fs::create_dir_all(&content).expect("a folder");
    std::fs::write(content.join("abcd1234"), "the large file\n").expect("a file");
    // What a removal that was stopped in the middle of the copy left.
    let part = bench.project.join(".git/lfs/objects/ab/cd");
    std::fs::create_dir_all(&part).expect("a folder");
    std::fs::write(part.join("abcd1234"), "the lar").expect("a file");
    let removed = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(removed, GitOutcome::Removed { .. }), "{removed:?}");
    let text = std::fs::read_to_string(part.join("abcd1234")).expect("the content");
    assert_eq!(text, "the large file\n");
}

#[cfg(unix)]
#[tokio::test]
async fn a_folder_that_cannot_be_read_is_not_taken_as_gone() {
    use std::os::unix::fs::PermissionsExt as _;

    let bench = Bench::new().await;
    std::fs::create_dir_all(bench.project.join("d")).expect("a folder");
    std::fs::write(bench.project.join("d/f"), "in the folder\n").expect("a file");
    bench.git_in(&bench.project, &["add", "d/f"]).await;
    let commit = ["commit", "--quiet", "--message=a folder"];
    bench.git_in(&bench.project, &commit).await;
    let folder = bench.worktree().await;
    let mode = |mode| std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(folder.join("d"), mode(0o000)).expect("a mode");
    let first = bench.call(PERSON, GitAction::Remove).await;
    std::fs::set_permissions(folder.join("d"), mode(0o755)).expect("a mode");
    assert!(refused(&first).is_some(), "{first:?}");
    let said = format!("{first:?}");
    assert!(said.contains("d/f"), "{said}");
    assert!(said.contains("cannot be read"), "{said}");
}
