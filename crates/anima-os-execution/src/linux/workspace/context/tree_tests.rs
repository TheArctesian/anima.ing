use super::*;

#[test]
fn capture_preserves_content_modes_links_and_v1_digest_without_private_directories() {
    let directory = std::env::temp_dir().join(format!("anima-context-{}", uuid::Uuid::now_v7()));
    let source = directory.join("source");
    let target = directory.join("target");
    fs::create_dir_all(source.join("d")).unwrap();
    fs::create_dir(&target).unwrap();
    fs::create_dir(source.join(".git")).unwrap();
    fs::write(
        source.join(".git/config"),
        b"synthetic excluded configuration",
    )
    .unwrap();
    fs::write(
        source.join(".env.fixture"),
        b"synthetic excluded credential",
    )
    .unwrap();
    fs::write(source.join("a.txt"), b"alpha\n").unwrap();
    fs::set_permissions(source.join("a.txt"), fs::Permissions::from_mode(0o640)).unwrap();
    fs::write(source.join("d/run"), b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(source.join("d/run"), fs::Permissions::from_mode(0o750)).unwrap();
    std::os::unix::fs::symlink("a.txt", source.join("link")).unwrap();
    let captured = Tree::capture(&source, &target).unwrap();
    assert_eq!(captured.entries, 4);
    // Stable context-v1 identity for this portable source tree.
    assert_eq!(
        captured.digest(),
        "sha256:ec69f255b5b6f4d00d1682d6ad227fd22e8aa1916788f68b1511b0f520109da5"
    );
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"alpha\n");
    assert_eq!(
        fs::read(target.join("d/run")).unwrap(),
        b"#!/bin/sh\nexit 0\n"
    );
    assert_eq!(
        fs::metadata(target.join("d/run"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::read_link(target.join("link")).unwrap(),
        Path::new("a.txt")
    );
    assert!(!target.join(".env.fixture").exists());
    assert!(!target.join(".git").exists());
    std::os::unix::fs::symlink("../outside", source.join("escape")).unwrap();
    let rejected = directory.join("rejected");
    fs::create_dir(&rejected).unwrap();
    assert!(Tree::capture(&source, &rejected).is_err());
    fs::remove_dir_all(directory).unwrap();
}
