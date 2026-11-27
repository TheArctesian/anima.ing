use std::path::Path;

use super::{Dir, Make, sync_probe};

#[test]
fn read_walks_skip_sync_and_write_walks_retry_the_directory_entry_barrier() {
    let temp = tempfile::tempdir().expect("temporary Git directory");
    let root = Dir::open(temp.path()).expect("open Git directory");
    let existing = Path::new("refs");
    let new = Path::new("lfs");
    std::fs::create_dir(temp.path().join(existing)).expect("existing directory");
    let failing = sync_probe::fail_for(temp.path(), || {});
    assert!(
        root.below(existing, Make::No)
            .expect("read succeeds")
            .is_some()
    );
    assert!(root.below(existing, Make::Missing).is_err());
    assert!(root.below(new, Make::Missing).is_err());
    assert!(
        temp.path().join(new).is_dir(),
        "mkdir happened before the failure"
    );
    assert!(
        root.below(new, Make::Missing).is_err(),
        "retry must still sync"
    );
    drop(failing);
    assert!(
        root.below(new, Make::Missing)
            .expect("durable retry")
            .is_some()
    );
}
