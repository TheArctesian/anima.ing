use super::*;

use std::io::Write as _;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

fn pending(root: &Path, contents: &[u8]) {
    let start = Instant::now();
    loop {
        let found = std::fs::read_dir(root).unwrap().flatten().any(|entry| {
            let path = entry.path();
            std::fs::read(&path).is_ok_and(|bytes| bytes == contents)
                || std::fs::read(path.join("content")).is_ok_and(|bytes| bytes == contents)
        });
        if found {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "a partial copy never appeared"
        );
        std::thread::yield_now();
    }
}

#[test]
fn concurrent_placements_publish_their_own_complete_files() {
    for object in ["pack-fixture.pack", "pack-fixture.idx", "lfs-object"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let (mut first, first_read) = UnixStream::pair().unwrap();
        let (mut second, second_read) = UnixStream::pair().unwrap();
        let copy = |reader: UnixStream| {
            let folder = Dir::open(root).unwrap();
            placed(&folder, object.as_ref(), File::from(OwnedFd::from(reader)))
        };
        std::thread::scope(|scope| {
            let a = scope.spawn(|| copy(first_read));
            first.write_all(b"first complete").unwrap();
            pending(root, b"first complete");
            let b = scope.spawn(|| copy(second_read));
            second.write_all(b"second").unwrap();
            pending(root, b"second");
            drop(first);
            let first_result = a.join().unwrap();
            let first_visible = std::fs::read(root.join(object));
            let written = second.write_all(b" complete");
            drop(second);
            let second_result = b.join().unwrap();
            first_result.unwrap();
            assert_eq!(first_visible.unwrap(), b"first complete");
            written.unwrap();
            second_result.unwrap();
            assert_eq!(
                std::fs::read(root.join(object)).unwrap(),
                b"second complete"
            );
            assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
        });
    }
}

#[test]
fn a_failed_publication_leaves_its_destination_and_allows_a_clean_retry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = root.join("source");
    let destination = root.join("published");
    std::fs::write(&source, b"complete pack").unwrap();
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("present"), b"existing").unwrap();
    let dir = Dir::open(root).unwrap();
    let write = || placed(&dir, "published".as_ref(), File::open(&source).unwrap());

    assert!(write().is_err());
    assert_eq!(
        std::fs::read(destination.join("present")).unwrap(),
        b"existing"
    );
    assert_eq!(
        root.read_dir().unwrap().count(),
        2,
        "no copy staging was left"
    );
    std::fs::rename(&destination, root.join("preserved")).unwrap();
    write().unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), b"complete pack");
}
