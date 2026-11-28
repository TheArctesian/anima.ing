use std::{
    fs::{self, Permissions},
    os::unix::fs::{PermissionsExt as _, chown, symlink},
};

use daemon_core::wire::DaemonToken;

use super::*;

const PERSON_UID: u32 = 65534;
const NEXT_UID: u32 = 65533;

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn only_a_fresh_enrollment_creates_the_restart_stable_authority() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = person_home(temp.path());
    let state = temp.path().join("authority");
    let open = || Enrollment::at(&state, home.clone(), PERSON_UID).unwrap();
    let original = credential("https://hub.example", 1);
    let renewed = credential("HTTPS://HUB.EXAMPLE:443", 2);
    let device = Uuid::from_u128(1);

    let enrollment = open();
    assert!(Enrollment::at(&state, home.clone(), PERSON_UID).is_err());
    refuses_without_fresh_binding(&enrollment, device);
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

fn refuses_without_fresh_binding(enrollment: &Enrollment, device: Uuid) {
    assert!(enrollment.read().unwrap().is_none());
    assert!(
        enrollment
            .save(&credential("https://hub.example", 2), None)
            .is_err()
    );
    assert!(
        enrollment
            .save(&credential("https://hub.example", 1), Some(Uuid::nil()))
            .is_err()
    );
    for hub in ["unix:/run/link.sock", "http://127.0.0.1:8443"] {
        assert!(enrollment.save(&credential(hub, 3), Some(device)).is_err());
    }
}

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn enrollment_refuses_unsafe_storage_and_a_substituted_home() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = person_home(temp.path());
    let moved = temp.path().join("moved");
    let state = temp.path().join("authority");
    fs::create_dir(&state).unwrap();
    let open = || Enrollment::at(&state, home.clone(), PERSON_UID);
    fs::set_permissions(&state, Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        open().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    fs::set_permissions(&state, Permissions::from_mode(0o700)).unwrap();
    chown(&state, Some(PERSON_UID), None).unwrap();
    assert!(open().is_err());
    chown(&state, Some(0), None).unwrap();
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
    chown(&home, Some(PERSON_UID), None).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(enrollment.save(&original, device).is_err());
    drop(enrollment);
    let enrollment = open().unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    drop(enrollment);
    let enrollment = Enrollment::at(&state, moved, PERSON_UID).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn enrollment_refuses_insecure_or_oversized_records() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = person_home(temp.path());
    let state = temp.path().join("authority");
    let enrollment = Enrollment::at(&state, home, PERSON_UID).unwrap();
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

#[test]
#[ignore = "requires Linux UID 0 and a writable /run"]
fn enrollment_refuses_an_in_place_owner_transfer() {
    let temp = tempfile::tempdir_in("/run").unwrap();
    let home = person_home(temp.path());
    let state = temp.path().join("authority");
    let open = |uid| Enrollment::at(&state, home.clone(), uid);
    let original = credential("https://hub.example", 1);
    let device = Some(Uuid::from_u128(1));

    assert_eq!(
        open(NEXT_UID).err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    chown(&home, Some(0), None).unwrap();
    assert_eq!(
        open(0).err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    chown(&home, Some(PERSON_UID), None).unwrap();
    let enrollment = open(PERSON_UID).unwrap();
    enrollment.save(&original, device).unwrap();

    chown(&home, Some(NEXT_UID), None).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        enrollment.save(&original, device).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    drop(enrollment);
    assert_eq!(
        open(PERSON_UID).err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );

    let enrollment = open(NEXT_UID).unwrap();
    assert_eq!(
        enrollment.read().err().unwrap().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        enrollment.save(&original, None).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    enrollment.save(&original, device).unwrap();
    assert!(enrollment.read().unwrap().is_some());
}

fn person_home(parent: &Path) -> PathBuf {
    let home = parent.join("person");
    fs::create_dir(&home).unwrap();
    chown(&home, Some(PERSON_UID), None).unwrap();
    home
}

fn credential(hub: &str, byte: u8) -> Vec<u8> {
    let token = DaemonToken::from_random([byte; 32]);
    format!(
        r#"{{"hub":"{hub}","token":"{}","expiresAt":"2030-01-01T00:00:00Z"}}"#,
        token.expose()
    )
    .into_bytes()
}
