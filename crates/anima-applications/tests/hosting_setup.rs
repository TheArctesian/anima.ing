#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::{
        fs::{MetadataExt as _, PermissionsExt as _},
        process::CommandExt as _,
    },
    path::Path,
    process::Command,
};

use anima_applications::{EngineKind, Settings, settings};

fn setup(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_anima-hosting-settings"));
    command
        .args(["--enable"])
        .arg(root)
        .args(["/fixture/docker", "/fixture/compose"]);
    command
}

fn status(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_anima-hosting-settings"));
    command.arg("--status").arg(root);
    command
}

fn assert_status(root: &Path, expected: &str) {
    let result = status(root).output().unwrap();
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        serde_json::from_slice::<String>(&result.stdout).unwrap(),
        expected
    );
}

#[test]
#[ignore = "requires the anima-daemon account and a disposable Linux root fixture"]
fn initial_hosting_settings_require_root_and_preserve_existing_state() {
    let root = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/run")
        .unwrap();
    let settings_file = root.path().join("settings.json");
    let denied = setup(root.path()).uid(65534).gid(65534).output().unwrap();
    assert!(!denied.status.success(), "{denied:?}");
    assert!(String::from_utf8_lossy(&denied.stderr).contains("as root"));
    assert!(!settings_file.exists());
    assert!(
        !status(root.path())
            .uid(65534)
            .gid(65534)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_status(root.path(), "not-configured");
    let result = setup(root.path()).output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let bytes = assert_initial_settings(&settings_file);
    assert_status(root.path(), "settings-saved");
    assert!(!setup(root.path()).status().unwrap().success());
    assert_eq!(fs::read_to_string(&settings_file).unwrap(), bytes);

    fs::remove_file(&settings_file).unwrap();
    let control = root.path().join("control");
    fs::create_dir(&control).unwrap();
    fs::set_permissions(&control, fs::Permissions::from_mode(0o700)).unwrap();
    let pair = control.join("hub-pair.json");
    fs::write(&pair, "existing pairing state").unwrap();
    fs::set_permissions(&pair, fs::Permissions::from_mode(0o600)).unwrap();
    assert_status(root.path(), "pairing-retained");
    let refused = setup(root.path()).output().unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("pairing state already exist"));
    assert!(!settings_file.exists());
    assert_eq!(fs::read_to_string(&pair).unwrap(), "existing pairing state");
    std::os::unix::fs::symlink(&pair, &settings_file).unwrap();
    assert!(!status(root.path()).output().unwrap().status.success());
}

fn assert_initial_settings(settings_file: &Path) -> String {
    let metadata = settings_file.metadata().unwrap();
    assert_eq!(metadata.uid(), 0);
    assert_eq!(metadata.mode() & 0o777, 0o600);
    let bytes = fs::read_to_string(settings_file).unwrap();
    let mut parsed = settings::read(&bytes).unwrap();
    let managed = parsed.managed.take().unwrap();
    assert_eq!(parsed, Settings::after_install());
    assert_eq!(managed.engine, EngineKind::System);
    assert_eq!(managed.socket, "/run/docker.sock");
    assert_eq!(managed.docker, "/fixture/docker");
    assert_eq!(managed.compose, "/fixture/compose");
    let daemon = nix::unistd::User::from_name("anima-daemon")
        .unwrap()
        .unwrap();
    assert_eq!(managed.accepted_user, daemon.uid.as_raw());
    bytes
}
