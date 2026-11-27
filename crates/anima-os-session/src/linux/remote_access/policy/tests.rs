use super::{DEFAULTS, Gid, Storage, Uid, fs};

#[test]
fn a_fresh_install_retries_interrupted_defaults_without_reopening_removed_access() {
    let folder = tempfile::tempdir().unwrap();
    let policy = folder.path().join("policy.json");
    let ports = folder.path().join("open-ports");
    let staging = folder.path().join("staging");
    let storage = Storage {
        policy: &policy,
        ports: &ports,
        staging: &staging,
    };
    let owner = (Uid::current(), Gid::current());

    assert!(storage.initialize(owner).is_err());
    assert!(policy.is_file());
    assert!(!ports.exists());
    fs::create_dir(&staging).unwrap();
    storage.initialize(owner).unwrap();
    assert_eq!(storage.ports().unwrap().as_deref(), Some(&DEFAULTS[..]));

    fs::remove_file(&ports).unwrap();
    storage.initialize(owner).unwrap();
    assert!(!ports.exists());
}

#[test]
fn existing_private_link_choices_win_before_or_during_default_initialization() {
    let owner = (Uid::current(), Gid::current());
    for (interrupted, existing) in [(false, &[22, 3389][..]), (true, &[][..])] {
        let folder = tempfile::tempdir().unwrap();
        let policy = folder.path().join("policy.json");
        let ports = folder.path().join("open-ports");
        let staging = folder.path().join("staging");
        let storage = Storage {
            policy: &policy,
            ports: &ports,
            staging: &staging,
        };
        if interrupted {
            assert!(storage.initialize(owner).is_err());
        }
        fs::create_dir(&staging).unwrap();
        let existing = serde_json::to_vec(existing).unwrap();
        fs::write(&ports, &existing).unwrap();

        storage.initialize(owner).unwrap();
        storage.initialize(owner).unwrap();
        assert_eq!(fs::read(ports).unwrap(), existing);
    }
}
