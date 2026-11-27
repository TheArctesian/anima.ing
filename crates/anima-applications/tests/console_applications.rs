#![cfg(target_os = "linux")]

use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::{Value, json};

const CURRENT: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e";
const INTERRUPTED: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9f";

fn write_private(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn deployed(root: &Path, application: &str) -> PathBuf {
    let project = root.join(application);
    let files = project.join("files");
    for directory in [&project, &files] {
        fs::create_dir(directory).unwrap();
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    write_private(&files.join("final-form.json"), "{}");
    write_private(&files.join("compose-as-it-came.yaml"), "services: {}");
    write_private(
        &files.join("environment-of-the-application"),
        "SYNTHETIC=fixture",
    );
    let digests = json!([{"service":"web","image":"example/web:1","digest":format!("sha256:{}", "a".repeat(64))}]);
    write_private(&files.join("digests.json"), &digests.to_string());
    files
}

fn observed(root: &Path, application: &str, id: &str) -> Value {
    let files = root.join(application).join("files");
    json!({
        "id":id.repeat(64), "project":format!("anima-{}", application.replace('-', "")),
        "service":"web", "folder":files, "finalForm":files.join("final-form.json"),
        "configuredImage":format!("example/web@sha256:{}", "a".repeat(64)),
    })
}

fn classify(root: &Path, containers: &Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_anima-applications"))
        .arg("console-applications")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(containers.to_string().as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{result:?}");
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
#[ignore = "requires a root-owned application store in a disposable Linux container"]
fn console_membership_requires_the_exact_settled_application() {
    let root = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/run")
        .unwrap();
    let root = root.path();
    deployed(root, CURRENT);
    let journal = deployed(root, INTERRUPTED).join(".version-transition");
    write_private(&journal, "starting:update\n");
    let mut wrong_location = observed(root, CURRENT, "b");
    wrong_location["finalForm"] = json!("/other/final-form.json");
    let mut wrong_service = observed(root, CURRENT, "d");
    wrong_service["service"] = json!("unknown");
    let mut older_image = observed(root, CURRENT, "e");
    older_image["configuredImage"] = json!(format!("example/web@sha256:{}", "b".repeat(64)));
    let mut no_inspection = observed(root, CURRENT, "f");
    no_inspection
        .as_object_mut()
        .unwrap()
        .remove("configuredImage");
    let result = classify(
        root,
        &json!([
            observed(root, CURRENT, "a"),
            wrong_location,
            observed(root, INTERRUPTED, "c"),
            wrong_service,
            older_image,
            no_inspection
        ]),
    );
    assert_eq!(
        result,
        json!({"a".repeat(64):CURRENT,"c".repeat(64):null,"f".repeat(64):null})
    );
    assert_eq!(fs::read_to_string(journal).unwrap(), "starting:update\n");
}
