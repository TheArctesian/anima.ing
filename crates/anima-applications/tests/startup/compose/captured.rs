use serde_json::Value;

use super::*;

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn a_captured_auto_remove_source_may_disappear_but_cannot_be_replaced() {
    let (host, socket) = fixture();
    let root = host.root.path();
    stage_import(root, DAEMON);
    fs::write(root.join("source-running"), "").unwrap();
    let captured = command(root, "capture-source").output().unwrap();
    assert!(captured.status.success(), "{captured:?}");
    let receipt: Value = serde_json::from_slice(&captured.stdout).unwrap();
    assert_eq!(receipt["schema"], 2);
    assert_eq!(receipt["source"]["containers"][0]["id"], SOURCE);
    assert_eq!(
        receipt["source"]["containers"][0]["name"],
        "systemd-example"
    );
    assert_eq!(
        receipt["source"]["containers"][0]["unit"],
        "container-example.service"
    );
    assert_eq!(receipt["source"]["containers"][0]["autoRemove"], true);

    fs::write(root.join("source-unit-state"), "masked").unwrap();
    fs::write(root.join("source-removed"), "").unwrap();
    fs::remove_file(root.join("source-running")).unwrap();
    let replay = command(root, "capture-source").output().unwrap();
    assert!(replay.status.success(), "{replay:?}");
    assert_eq!(replay.stdout, captured.stdout);
    let stage = root.join("applications/imports").join(IMPORT);
    let interrupted = stage.join(".write-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    fs::write(&interrupted, b"partial capture").unwrap();
    fs::set_permissions(interrupted, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(command(root, "import-binds").status().unwrap().success());
    let server = Server::start(root, &socket);
    assert_held(&server);

    fs::write(root.join("source-unit-state"), "masked-runtime").unwrap();
    refused_confirmation(authorize(root), "persistently masked");
    fs::write(root.join("source-unit-state"), "masked").unwrap();
    let authorized = authorize(root);
    assert!(authorized.status.success(), "{authorized:?}");
    let authorization: Value = serde_json::from_slice(&authorized.stdout).unwrap();
    assert_eq!(authorization["schema"], 2);

    fs::write(root.join("recreate-on-pull"), "").unwrap();
    assert_no_start(root, &server);
    fs::remove_file(root.join("recreate-on-pull")).unwrap();
    fs::remove_file(root.join("source-replaced")).unwrap();
    let digest = start_candidate(root, &server);
    let result = confirm(root, &digest, "--verified-persistent-write");
    assert!(result.status.success(), "{result:?}");
    let final_record: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(final_record["schema"], 2);
    assert_eq!(
        final_record["confirmation"]["authoritativeRuntime"],
        "docker"
    );
}
