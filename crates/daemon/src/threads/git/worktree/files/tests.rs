//! Tests of the copies between a worktree and a folder of the runner.

use super::*;

fn folders() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let keep = tempfile::tempdir().expect("a folder");
    let top = std::fs::canonicalize(keep.path()).expect("its real path");
    let (worktree, own, outside) = (top.join("worktree"), top.join("own"), top.join("outside"));
    for folder in [&worktree, &own, &outside] {
        std::fs::create_dir_all(folder).expect("a folder");
    }
    std::fs::write(outside.join("secret"), "SECRET\n").expect("a file");
    (keep, worktree, own, outside)
}

#[test]
fn a_path_with_dots_or_a_git_folder_is_not_inside() {
    for bad in [
        "",
        "../x",
        "/etc/hosts",
        "a/../b",
        ".git/config",
        "a/.GIT/x",
        "./a",
    ] {
        assert!(!inside(Path::new(bad)), "{bad}");
    }
    assert!(inside(Path::new("src/a file.rs")));
    assert!(inside(Path::new("-odd:name")));
}

#[test]
fn a_link_is_copied_as_a_link_and_a_folder_that_is_a_link_is_not_passed() {
    let (_keep, worktree, own, outside) = folders();
    std::fs::write(worktree.join("plain.txt"), "plain\n").expect("a file");
    std::os::unix::fs::symlink(outside.join("secret"), worktree.join("link")).expect("a link");
    std::os::unix::fs::symlink(&outside, worktree.join("dir")).expect("a link");
    let root = Root::open(&worktree).expect("the worktree");
    let into = Dir::open(&own).expect("the folder of the runner");
    let take = |path: &str| root.take(Path::new(path), &into, &mut { u64::MAX });
    assert!(take("plain.txt").expect("a copy"));
    // A second name for the same file (a disk that does not tell `A` from `a`) is no
    // failure: the copy is there.
    assert!(take("plain.txt").expect("the copy that is there"));
    // A different file at the name of a copy is refused, not taken as copied.
    std::fs::write(worktree.join("plain.txt"), "other\n").expect("a file");
    let different = take("plain.txt").expect_err("a different file passed as the copy");
    assert_eq!(different.kind(), std::io::ErrorKind::InvalidData);
    assert!(take("link").expect("a copy"));
    assert!(own.join("link").is_symlink(), "the link was followed");
    // `dir` is a link: nothing below it is a file of the worktree.
    assert!(!take("dir/secret").expect("no copy"));
    assert!(!own.join("dir").exists());
    assert!(!take("gone").expect("no copy"));
    // A copy is never written through a link that an earlier copy put there, and never
    // over a copy that is there.
    std::fs::create_dir_all(worktree.join("d")).expect("a folder");
    std::fs::write(worktree.join("d/f"), "PAYLOAD\n").expect("a file");
    let through = root.take_to(Path::new("d/f"), (&into, Path::new("link")), &mut {
        u64::MAX
    });
    assert!(through.is_err(), "a copy went over a link");
    let below = root.take_to(Path::new("d/f"), (&into, Path::new("link/f")), &mut {
        u64::MAX
    });
    assert!(below.is_err(), "a copy went through a link");
    let secret = std::fs::read_to_string(outside.join("secret")).expect("the file");
    assert_eq!(secret, "SECRET\n");
    assert!(!outside.join("f").exists());
    assert!(Root::open(&worktree.join("dir")).is_err());
}

#[test]
fn a_file_is_never_written_or_removed_through_a_link() {
    let (_keep, worktree, own, outside) = folders();
    std::fs::create_dir_all(own.join("d")).expect("a folder");
    std::fs::write(own.join("d/f"), "PAYLOAD\n").expect("a file");
    std::fs::write(own.join("top"), "PAYLOAD\n").expect("a file");
    // What a process of an agent leaves: a link where a folder and where a file should be.
    std::os::unix::fs::symlink(&outside, worktree.join("d")).expect("a link");
    std::os::unix::fs::symlink(outside.join("secret"), worktree.join("top")).expect("a link");
    let root = Root::open(&worktree).expect("the worktree");
    root.put(Path::new("d/f"), &own).expect("a write");
    root.put(Path::new("top"), &own).expect("a write");
    assert!(!outside.join("f").exists(), "a file was written outside");
    let secret = std::fs::read_to_string(outside.join("secret")).expect("the file");
    assert_eq!(
        secret, "SECRET\n",
        "a file outside was written through a link"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("d/f")).expect("the file"),
        "PAYLOAD\n"
    );
    assert!(
        worktree
            .join("d")
            .symlink_metadata()
            .expect("a folder")
            .is_dir()
    );
    std::fs::remove_dir_all(worktree.join("d")).expect("the folder");
    std::os::unix::fs::symlink(&outside, worktree.join("d")).expect("a link");
    root.remove(Path::new("d/secret"))
        .expect("nothing to remove");
    assert!(
        outside.join("secret").exists(),
        "a file outside was removed"
    );
    root.remove(Path::new("top")).expect("the removal");
    assert!(!worktree.join("top").exists());
}

#[test]
fn a_removed_file_takes_its_empty_folders_with_it() {
    let (_keep, worktree, _own, _outside) = folders();
    std::fs::create_dir_all(worktree.join("a/b")).expect("folders");
    std::fs::write(worktree.join("a/b/f"), "x").expect("a file");
    std::fs::write(worktree.join("a/keep"), "x").expect("a file");
    let root = Root::open(&worktree).expect("the worktree");
    root.remove(Path::new("a/b/f")).expect("the removal");
    assert!(!worktree.join("a/b").exists());
    assert!(worktree.join("a/keep").exists());
}

#[test]
fn a_name_that_is_too_long_for_the_disk_is_no_file_and_no_failure() {
    let (_keep, worktree, own, _outside) = folders();
    let root = Root::open(&worktree).expect("the worktree");
    let into = Dir::open(&own).expect("the folder of the runner");
    let long = "x".repeat(300);
    for path in [format!("{long}/f"), long.clone()] {
        let taken = root.take(Path::new(&path), &into, &mut { u64::MAX });
        assert!(!taken.expect("no failure"), "{path}");
    }
    assert!(shown(Path::new(&format!("{long}/end\n"))).ends_with("/end\\n"));
}

#[test]
fn a_socket_at_the_name_of_a_file_is_no_file_and_no_failure() {
    let (_keep, worktree, own, _outside) = folders();
    // A short path: the name of a socket has a small limit.
    let short = tempfile::tempdir_in("/tmp").expect("a folder");
    let at = short.path().join("s");
    let _socket = std::os::unix::net::UnixListener::bind(&at).expect("a socket");
    std::fs::rename(&at, worktree.join("socket")).expect("the move");
    let root = Root::open(&worktree).expect("the worktree");
    let into = Dir::open(&own).expect("the folder of the runner");
    let taken = root.take(Path::new("socket"), &into, &mut { u64::MAX });
    assert!(!taken.expect("no failure"));
}
