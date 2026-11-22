//! The checks of names and paths.

use super::*;

const THREAD: Uuid = Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);

#[test]
fn the_branch_of_a_thread_has_only_the_prefix_and_hex_digits() {
    assert_eq!(thread_branch(THREAD), "anima/12345678");
}

#[test]
fn a_folder_name_keeps_the_fixed_alphabet_and_never_hides() {
    assert_eq!(folder_name("anima.ing"), "anima.ing");
    assert_eq!(folder_name("../..//etc"), "etc");
    assert_eq!(folder_name("my repo; rm -rf"), "my-repo--rm--rf");
    assert_eq!(folder_name("..."), "repository");
}

#[test]
fn a_branch_name_that_git_could_read_as_an_option_or_a_range_is_refused() {
    for bad in [
        "",
        "-f",
        "--upload-pack=x",
        "a..b",
        "a b",
        "a:b",
        "a~1",
        "a^",
        "HEAD",
        "/a",
        "a/",
        "a//b",
        "a/.b",
        "refs/x.lock",
        "x@{1}",
    ] {
        assert_eq!(branch_name(bad), Err(BadPlace::BadName), "{bad}");
    }
    assert_eq!(
        branch_name("feature/git-stage_2.0"),
        Ok("feature/git-stage_2.0")
    );
}

#[test]
fn a_path_that_leaves_the_repository_is_refused() {
    for bad in [
        "",
        "../x",
        "a/../../x",
        "/etc/passwd",
        "-rf",
        ":(top)x",
        ".git/config",
        "a/.GIT/hooks/x",
        "a\0b",
        "./a",
    ] {
        assert_eq!(inside(bad), Err(BadPlace::Escapes), "{bad:?}");
    }
    assert_eq!(inside("src/lib.rs"), Ok("src/lib.rs"));
    assert_eq!(inside("a file.txt"), Ok("a file.txt"));
}

#[test]
fn a_repository_name_with_a_path_in_it_is_not_a_place() {
    let home = Home::at(Path::new("/tmp/worktrees"), Path::new("/tmp/records")).expect("a home");
    assert_eq!(home.planned(THREAD, "../other"), Err(BadPlace::BadName));
    assert_eq!(home.planned(THREAD, "a/b"), Err(BadPlace::BadName));
    let records = Path::new("/tmp/records");
    assert!(Home::at(Path::new("relative"), records).is_err());
    assert!(Home::at(Path::new("/tmp/../etc"), records).is_err());
    assert!(Home::at(Path::new("/tmp/w"), Path::new("/tmp/w/records")).is_err());
}
