use std::{
    fs::{self, Permissions},
    os::unix::fs::{PermissionsExt as _, symlink},
};

use daemon_core::wire::DaemonToken;

use super::*;

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn only_a_fresh_enrollment_creates_the_restart_stable_authority() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = temp.path().join("person");
    fs::create_dir(&home).unwrap();
    let state = temp.path().join("authority");
    let open = || Enrollment::at(&state, home.clone()).unwrap();
    let original = credential("https://hub.example", 1);
    let renewed = credential("HTTPS://HUB.EXAMPLE:443", 2);
    let device = Uuid::from_u128(1);

    let enrollment = open();
    assert!(Enrollment::at(&state, home.clone()).is_err());
    assert!(enrollment.read().unwrap().is_none());
    assert!(enrollment.save(&renewed, None).is_err());
    assert!(enrollment.save(&original, Some(Uuid::nil())).is_err());
    for hub in ["unix:/run/link.sock", "http://127.0.0.1:8443"] {
        assert!(enrollment.save(&credential(hub, 3), Some(device)).is_err());
    }
    enrollment.save(&original, Some(device)).unwrap();
    let first = enrollment.read().unwrap().unwrap();
    assert_eq!(*first.record, original);
    assert_eq!(first.hub.as_text(), "https://hub.example");
    assert_eq!(first.device, device);
    assert!(
        enrollment
            .save(&credential("https://other.example", 3), None)
            .is_err()
    );
    enrollment.save(&renewed, None).unwrap();
    drop(enrollment);

    let enrollment = open();
    let persisted = enrollment.read().unwrap().unwrap();
    assert_eq!(*persisted.record, renewed);
    assert_eq!(persisted.device, device);
    assert_eq!(*persisted.seed, *first.seed);
    let replacement = Uuid::from_u128(2);
    enrollment.save(&original, Some(replacement)).unwrap();
    let re_enrolled = enrollment.read().unwrap().unwrap();
    assert_eq!(re_enrolled.device, replacement);
    assert_ne!(*re_enrolled.seed, *persisted.seed);
    enrollment.remove().unwrap();
    enrollment.remove().unwrap();
    drop(enrollment);
    let enrollment = open();
    assert!(enrollment.read().unwrap().is_none());
    assert!(enrollment.save(&renewed, None).is_err());
}

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn enrollment_refuses_unsafe_storage_and_a_substituted_home() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = temp.path().join("person");
    let moved = temp.path().join("moved");
    let state = temp.path().join("authority");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&state).unwrap();
    let open = || Enrollment::at(&state, home.clone());
    fs::set_permissions(&state, Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        open().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    fs::set_permissions(&state, Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(temp.path(), Permissions::from_mode(0o722)).unwrap();
    assert_eq!(
        open().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    fs::set_permissions(temp.path(), Permissions::from_mode(0o700)).unwrap();
    let enrollment = open().unwrap();
    let original = credential("https://hub.example", 1);
    let device = Some(Uuid::from_u128(1));
    enrollment.save(&original, device).unwrap();

    fs::rename(&home, &moved).unwrap();
    symlink(&moved, &home).unwrap();
    assert!(enrollment.read().is_err());
    fs::remove_file(&home).unwrap();
    fs::create_dir(&home).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(enrollment.save(&original, device).is_err());
    drop(enrollment);
    let enrollment = Enrollment::at(&state, moved).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn enrollment_refuses_insecure_or_oversized_records() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = temp.path().join("person");
    let state = temp.path().join("authority");
    fs::create_dir(&home).unwrap();
    let enrollment = Enrollment::at(&state, home).unwrap();
    let original = credential("https://hub.example", 1);
    enrollment
        .save(&original, Some(Uuid::from_u128(1)))
        .unwrap();
    let path = state.join(ENROLLMENT);
    let copy = state.join("copy");

    fs::hard_link(&path, &copy).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    fs::remove_file(&path).unwrap();
    symlink(&copy, &path).unwrap();
    assert!(enrollment.read().is_err());
    fs::remove_file(&path).unwrap();
    fs::rename(&copy, &path).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    assert_eq!(*enrollment.read().unwrap().unwrap().record, original);
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(64 * 1024)
        .unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
}

fn credential(hub: &str, byte: u8) -> Vec<u8> {
    let token = DaemonToken::from_random([byte; 32]);
    format!(
        r#"{{"hub":"{hub}","token":"{}","expiresAt":"2030-01-01T00:00:00Z"}}"#,
        token.expose()
    )
    .into_bytes()
}
