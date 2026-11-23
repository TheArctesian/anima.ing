//! The owner rule of the folders of found models (Section 2.3; PULL-012, R-443).

use crate::model_work::found::owner::{Entry, Reader, owner_permits};

const fn entry(owner: u32, group: u32, mode: u32) -> Entry {
    Entry { owner, group, mode }
}

#[test]
fn pull_012_an_entry_that_another_user_can_write_is_not_read() {
    let (daemon, ollama) = (1000, 996);
    let reader = Reader {
        user: daemon,
        private_group: None,
    };

    assert!(owner_permits(entry(daemon, daemon, 0o755), reader, daemon));
    assert!(owner_permits(entry(ollama, ollama, 0o755), reader, ollama));
    assert!(owner_permits(entry(0, 0, 0o644), reader, daemon));
    assert!(!owner_permits(entry(1001, 1001, 0o755), reader, daemon));
    assert!(!owner_permits(entry(daemon, daemon, 0o757), reader, daemon));
    assert!(!owner_permits(entry(daemon, daemon, 0o775), reader, daemon));
}

#[test]
fn pull_012_only_the_private_group_of_the_daemon_may_write_an_entry() {
    let (daemon, shared) = (1000, 1500);
    let reader = Reader {
        user: daemon,
        private_group: Some(daemon),
    };

    assert!(owner_permits(entry(daemon, daemon, 0o775), reader, daemon));
    assert!(owner_permits(entry(daemon, daemon, 0o664), reader, daemon));
    assert!(owner_permits(entry(0, daemon, 0o664), reader, daemon));
    assert!(!owner_permits(entry(daemon, shared, 0o775), reader, daemon));
    assert!(!owner_permits(entry(daemon, daemon, 0o777), reader, daemon));
    assert!(!owner_permits(entry(1001, daemon, 0o775), reader, daemon));
}
