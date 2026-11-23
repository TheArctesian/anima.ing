//! The refs of a repository as files: names, reading, and the move of a branch.

use super::*;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";

fn common() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a folder");
    std::fs::create_dir_all(folder.path().join("refs/heads")).expect("refs");
    folder
}

#[test]
fn a_name_that_is_no_ref_or_leaves_the_refs_is_refused() {
    for bad in [
        "HEAD",
        "refs/",
        "refs/../config",
        "refs/heads/a b",
        "config",
        "refs/heads/x.lock",
    ] {
        assert_eq!(name(bad), None, "{bad}");
    }
    assert_eq!(
        name("refs/heads/anima/12345678"),
        Some("refs/heads/anima/12345678")
    );
    assert_eq!(id(A), Some(A));
    assert_eq!(id("HEAD"), None);
    assert_eq!(id(&A.to_uppercase().replace('1', "A")), None);
}

#[test]
fn a_loose_ref_wins_over_a_packed_one_and_a_ref_that_names_a_ref_is_followed() {
    let folder = common();
    let packed = format!("# pack-refs with: peeled\n{A} refs/heads/main\n^{B}\n{A} refs/tags/v1\n");
    std::fs::write(folder.path().join("packed-refs"), packed).expect("packed");
    let found = |name| resolve(folder.path(), name).expect("readable");
    assert_eq!(found("refs/heads/main").as_deref(), Some(A));
    assert_eq!(found("refs/heads/none"), None);
    std::fs::write(folder.path().join("refs/heads/main"), format!("{B}\n")).expect("loose");
    assert_eq!(found("refs/heads/main").as_deref(), Some(B));
    std::fs::create_dir_all(folder.path().join("refs/remotes/origin")).expect("a folder");
    let head = folder.path().join("refs/remotes/origin/HEAD");
    std::fs::write(head, "ref: refs/heads/main\n").expect("a ref");
    assert_eq!(found("refs/remotes/origin/HEAD").as_deref(), Some(B));
    assert_eq!(
        names(folder.path(), "refs/remotes/origin/HEAD")
            .expect("readable")
            .as_deref(),
        Some("refs/heads/main")
    );
}

#[test]
fn a_branch_moves_only_from_where_it_was() {
    let folder = common();
    let put = |to, was| put(folder.path(), "refs/heads/anima/x", to, was);
    assert!(matches!(put(B, Was::At(A)), Err(ProjectError::Moved)));
    put(A, Was::Absent).expect("a new branch");
    assert!(matches!(put(B, Was::Absent), Err(ProjectError::Moved)));
    assert!(matches!(put(B, Was::At(B)), Err(ProjectError::Moved)));
    put(B, Was::At(A)).expect("a move");
    let file = folder.path().join("refs/heads/anima/x");
    assert_eq!(
        std::fs::read_to_string(&file).expect("a ref"),
        format!("{B}\n")
    );
    assert!(!folder.path().join("refs/heads/anima/x.lock").exists());
}

#[test]
fn a_ref_that_git_holds_is_not_written() {
    let folder = common();
    std::fs::write(folder.path().join("refs/heads/x.lock"), "").expect("a lock");
    let held = put(folder.path(), "refs/heads/x", A, Was::Absent);
    assert!(matches!(held, Err(ProjectError::Locked(_))));
    assert!(!folder.path().join("refs/heads/x").exists());
}

#[cfg(unix)]
#[test]
fn a_link_in_the_refs_does_not_send_a_write_out_of_them() {
    let folder = common();
    let outside = tempfile::tempdir().expect("a folder");
    let link = folder.path().join("refs/heads/anima");
    std::os::unix::fs::symlink(outside.path(), &link).expect("a link");
    let sent = put(folder.path(), "refs/heads/anima/x", A, Was::Absent);
    assert!(matches!(sent, Err(ProjectError::Escapes)));
    assert_eq!(
        std::fs::read_dir(outside.path()).expect("a folder").count(),
        0
    );
}
