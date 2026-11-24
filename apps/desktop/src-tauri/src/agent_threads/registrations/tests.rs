use super::*;
use std::{fs, os::unix::fs::symlink};

#[test]
fn watch_open_refuses_redirected_components_and_special_files() {
    let home = tempfile::tempdir().expect("made home");
    let outside = tempfile::tempdir().expect("made outside");
    let file = outside.path().join("session.jsonl");
    fs::write(&file, "synthetic").expect("made session");
    let metadata = fs::metadata(&file).expect("made metadata");
    let identity = (metadata.dev(), metadata.ino());
    symlink(outside.path(), home.path().join(".claude")).expect("redirected parent");
    assert!(
        open(
            home.path(),
            &home.path().join(".claude/session.jsonl"),
            identity
        )
        .is_err()
    );
    symlink(&file, home.path().join("session.jsonl")).expect("redirected file");
    assert!(open(home.path(), &home.path().join("session.jsonl"), identity).is_err());
    let pipe = home.path().join("pipe.jsonl");
    nix::unistd::mkfifo(&pipe, Mode::S_IRUSR | Mode::S_IWUSR).expect("made pipe");
    let metadata = fs::metadata(&pipe).expect("pipe metadata");
    assert!(open(home.path(), &pipe, (metadata.dev(), metadata.ino())).is_err());
}

#[test]
fn discovery_refuses_an_unbounded_registration_set() {
    let home = tempfile::tempdir().expect("made home");
    let root = home.path().join(".claude/projects");
    fs::create_dir_all(&root).expect("made projects");
    for index in 0..1024 {
        fs::create_dir(root.join(index.to_string())).expect("made project");
    }
    assert!(folders::discover(home.path()).is_err());
}
