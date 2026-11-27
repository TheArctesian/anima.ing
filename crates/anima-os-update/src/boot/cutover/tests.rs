use std::{fs, os::unix::fs::PermissionsExt as _, path::Path};

use anima_os_maintenance::cutover::{Format, require_entry, require_generation, required_format};

#[test]
fn retained_cutovers_require_an_explicit_compatible_generation() {
    let application_root = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    fs::set_permissions(application_root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(required_format(application_root.path()).unwrap(), None);
    let ledger = application_root.path().join("cutovers");
    fs::create_dir(&ledger).unwrap();
    fs::set_permissions(&ledger, fs::Permissions::from_mode(0o700)).unwrap();
    let id = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e";
    let record = ledger.join(format!("{id}.json"));
    authorization(&record, id, 1);
    assert_eq!(
        required_format(application_root.path()).unwrap(),
        Some(Format::Retained)
    );
    assert!(require_generation(system.path(), Format::Retained).is_err());

    fs::write(system.path().join("anima-hosting-cutover"), b"1\n").unwrap();
    require_generation(system.path(), Format::Retained).unwrap();
    let second_id = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9f";
    let captured = ledger.join(format!("{second_id}.json"));
    authorization(&captured, second_id, 2);
    let required = required_format(application_root.path()).unwrap().unwrap();
    assert_eq!(required, Format::Captured);
    assert!(require_generation(system.path(), required).is_err());
    fs::write(system.path().join("anima-hosting-cutover-captured"), b"2\n").unwrap();
    require_generation(system.path(), required).unwrap();

    authorization(&record, id, 3);
    assert!(required_format(application_root.path()).is_err());
}

#[test]
fn an_entry_cannot_select_a_live_init_different_from_the_checked_generation() {
    let entries = tempfile::tempdir().unwrap();
    let fallback = entries.path().join("anima-current.conf");
    let valid = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-nixos-system-anima-approved";
    let old = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-nixos-system-anima-old";
    let mismatch = [
        format!("title fallback\noptions init={old}/init quiet\n"),
        format!("title fallback\noptions init={valid}/init quiet\nOptions init={old}/init\n"),
    ];
    for entry in mismatch {
        fs::write(&fallback, entry).unwrap();
        let refused = require_entry(&fallback, Some(valid), Some(Format::Retained)).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("does not match the intended generation")
        );
    }
}

fn authorization(path: &Path, id: &str, schema: u8) {
    fs::write(
        path,
        serde_json::json!({"application":id,"schema":schema}).to_string(),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
