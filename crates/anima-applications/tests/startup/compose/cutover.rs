#[path = "captured.rs"]
mod captured;

use std::{
    fmt::Write as _,
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
    path::Path,
    process::{Command, Output},
};

use super::{
    COMPOSE, DAEMON, DEVICE, ENVIRONMENT, HUB, Server, applied, apply, deploy, executable, fixture,
    installed_files, json,
};
use sha2::{Digest as _, Sha256};

const IMPORT: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9f";
const SOURCE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn an_import_needs_local_authority_and_checks_the_source_again_before_docker_starts() {
    for source_uid in [0, DAEMON] {
        verify_local_authority(source_uid);
    }
}

fn verify_local_authority(source_uid: u32) {
    let (host, socket) = fixture();
    let root = host.root.path();
    prepare_import(root, source_uid);
    let server = Server::start(root, &socket);
    assert_held(&server);

    fs::write(root.join("with-source-unit"), "").unwrap();
    for state in ["enabled", "disabled", "masked-runtime", "masked-active"] {
        fs::write(root.join("source-unit-state"), state).unwrap();
        refused_confirmation(authorize(root), "persistently masked");
    }
    fs::write(root.join("source-unit-state"), "masked").unwrap();
    fs::write(root.join("source-restart"), "").unwrap();
    assert!(!authorize(root).status.success());
    fs::remove_file(root.join("source-restart")).unwrap();
    let recorded = authorize(root);
    assert!(recorded.status.success(), "{recorded:?}");
    let proof: serde_json::Value = serde_json::from_slice(&recorded.stdout).unwrap();
    assert_eq!(proof["externalStartersDisabled"], true);
    assert_eq!(recorded.stdout, authorize(root).stdout);

    fs::write(root.join("source-unit-state"), "enabled").unwrap();
    assert_held(&server);
    fs::write(root.join("source-unit-state"), "masked").unwrap();

    fs::write(root.join("different-store"), "").unwrap();
    assert_held(&server);
    fs::remove_file(root.join("different-store")).unwrap();
    fs::write(root.join("unmask-on-pull"), "").unwrap();
    assert_no_start(root, &server);
    fs::remove_file(root.join("unmask-on-pull")).unwrap();
    fs::write(root.join("source-unit-state"), "masked").unwrap();
    check_last_start(root, &server);
}

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn only_the_healthy_verified_imported_version_can_commit_authority() {
    let (host, socket) = fixture();
    let root = host.root.path();
    prepare_import(root, 0);
    let server = Server::start(root, &socket);
    let authorization = authorize(root);
    assert!(authorization.status.success(), "{authorization:?}");
    let stale = "f".repeat(64);
    refused_confirmation(
        confirm(root, &stale, "--verified-persistent-write"),
        "confirmed Compose version",
    );
    let digest = start_candidate(root, &server);
    refused_confirmation(
        confirm(root, &digest, "--unverified"),
        "verify a persistent application write",
    );
    refused_confirmation(
        confirm(root, &stale, "--verified-persistent-write"),
        "confirmed Compose version",
    );
    for marker in [
        "unhealthy-docker",
        "wrong-docker-bind",
        "stale-docker-config",
        "source-running",
    ] {
        fs::write(root.join(marker), "").unwrap();
        let outcome = if marker == "source-running" {
            "source containers or runtime could not be verified"
        } else {
            "Docker candidate is not healthy or its running configuration, image or transferred mounts do not match"
        };
        refused_confirmation(
            confirm(root, &digest, "--verified-persistent-write"),
            outcome,
        );
        fs::remove_file(root.join(marker)).unwrap();
        assert_eq!(authorize(root).stdout, authorization.stdout);
    }
    assert_confirmation(root, &server, &digest, &stale);
}

fn start_candidate(root: &Path, server: &Server) -> String {
    let compose = format!(
        "{COMPOSE}    volumes:\n      - type: bind\n        source: binds/state\n        target: /persisted\n"
    );
    let mut request = apply(true);
    let mut work: serde_json::Value =
        serde_json::from_str(request["json"].as_str().unwrap()).unwrap();
    work["parameters"]["fileSha256"] = json!(sha256(compose.as_bytes()));
    request["json"] = json!(work.to_string());
    assert_eq!(
        server.exchange((DAEMON, HUB), &request, ("start", &[&compose, ENVIRONMENT])),
        applied()
    );
    let bytes = fs::read(installed_files(root).join("final-form.json")).unwrap();
    sha256(&bytes)
}

fn sha256(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}

fn assert_confirmation(root: &Path, server: &Server, digest: &str, stale: &str) {
    let confirmation = confirm(root, digest, "--verified-persistent-write");
    assert!(confirmation.status.success(), "{confirmation:?}");
    let record: serde_json::Value = serde_json::from_slice(&confirmation.stdout).unwrap();
    let receipt = &record["confirmation"];
    assert_eq!(receipt["authoritativeRuntime"], "docker");
    assert_eq!(receipt["finalFormSha256"], digest);
    assert_eq!(receipt["persistentWriteVerified"], true);
    assert_eq!(receipt["health"], "healthy");
    let path = root.join("applications").join(DEVICE).join("binds");
    assert_eq!(receipt["bindRoot"], path.to_str().unwrap());
    assert_eq!(
        receipt["dockerSocket"],
        root.join("engine.sock").to_str().unwrap()
    );
    assert_eq!(authorize(root).stdout, confirmation.stdout);
    fs::write(root.join("source-running"), "").unwrap();
    assert_held(server);
    // The committed receipt replays even when the candidate can no longer be probed.
    fs::rename(root.join("settings.json"), root.join("disabled-settings")).unwrap();
    let replay = confirm(root, digest, "--verified-persistent-write");
    assert!(replay.status.success(), "{replay:?}");
    assert_eq!(replay.stdout, confirmation.stdout);
    refused_confirmation(
        confirm(root, stale, "--verified-persistent-write"),
        "confirmed Compose version",
    );
}

fn refused_confirmation(output: Output, message: &str) {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(message), "{stderr}");
}

fn confirm(root: &Path, digest: &str, flag: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_anima-applications"))
        .arg("confirm-cutover")
        .arg(root.join("settings.json"))
        .arg(root.join("docker-config"))
        .arg(root.join("applications"))
        .args([DEVICE, IMPORT, digest, flag])
        .env("ANIMA_LEGACY_BIN", root.join("legacy-bin"))
        .output()
        .unwrap()
}

fn assert_held(server: &Server) {
    assert_eq!(
        server.send(DAEMON, HUB, &apply(true)),
        super::super::refused("migration_held")
    );
}

fn check_last_start(root: &Path, server: &Server) {
    fs::write(root.join("revive-on-pull"), "").unwrap();
    assert_no_start(root, server);
    fs::remove_file(root.join("revive-on-pull")).unwrap();
    fs::remove_file(root.join("source-running")).unwrap();
    assert_eq!(deploy(server, true, ENVIRONMENT), applied());
}

fn assert_no_start(root: &Path, server: &Server) {
    assert_eq!(
        deploy(server, true, ENVIRONMENT),
        json!({"outcome":"failed","cause":"migration_held"})
    );
    assert!(!root.join("started").exists());
    assert_held(server);
}

fn prepare_import(root: &Path, source_uid: u32) {
    stage_import(root, source_uid);
    assert!(command(root, "import-binds").status().unwrap().success());
}

fn stage_import(root: &Path, source_uid: u32) {
    let imports = root.join("applications/imports");
    let stage = imports.join(IMPORT);
    for directory in [
        imports,
        stage.clone(),
        stage.join("binds"),
        stage.join("binds/state"),
    ] {
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let manifest = stage.join("migration.json");
    fs::write(
        &manifest,
        json!({
            "schema":1, "application":DEVICE, "importId":IMPORT,
            "backupId":"0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8da0",
            "podmanUid":source_uid, "podmanContainers":[SOURCE], "binds":["state"]
        })
        .to_string(),
    )
    .unwrap();
    fs::set_permissions(manifest, fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir(root.join("legacy-bin")).unwrap();
    for (name, script) in [("podman", PODMAN), ("systemctl", SYSTEMCTL)] {
        let path = root.join("legacy-bin").join(name);
        executable(&path, script);
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    symlink("/usr/sbin/runuser", root.join("legacy-bin/runuser")).unwrap();
}

fn authorize(root: &Path) -> Output {
    command(root, "authorize-cutover")
        .arg("--source-startup-disabled")
        .output()
        .unwrap()
}

fn command(root: &Path, operation: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_anima-applications"));
    command
        .arg(operation)
        .arg(root.join("applications"))
        .args([DEVICE, IMPORT])
        .env("ANIMA_LEGACY_BIN", root.join("legacy-bin"));
    command
}

const PODMAN: &str = include_str!("podman.sh");

const SYSTEMCTL: &str = include_str!("systemctl.sh");
