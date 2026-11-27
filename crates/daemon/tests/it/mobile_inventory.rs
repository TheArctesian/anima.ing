//! The real isolated worker with synthetic tools and a synthetic SDK root only.
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

fn script(folder: &Path, name: &str, body: &str) {
    let path = folder.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn read(folder: &Path, kind: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_anima-daemon"))
        .args(["__mobile-inventory", kind])
        .env_clear()
        .env("PATH", folder)
        .current_dir(folder)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn mobile_inventory_missing_tools_and_fake_android_version_are_truthful() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(read(root.path(), "emulator"), json!({"state":"missing"}));
    assert_eq!(read(root.path(), "sdk"), json!({"state":"missing"}));
    script(
        root.path(),
        "emulator",
        "[ \"$1\" = '-version' ] || exit 1\nprintf 'Android emulator version 35.2.10.0 (build_id 1)\\n'",
    );
    assert_eq!(
        read(root.path(), "emulator"),
        json!({"state":"read","version":"35.2.10.0"})
    );
    script(root.path(), "emulator", "exit 1");
    assert_eq!(read(root.path(), "emulator"), json!({"state":"exited"}));
}

#[test]
#[cfg(target_os = "macos")]
fn mobile_inventory_apple_probes_only_vendor_catalogues_after_developer_tool_check() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(read(root.path(), "runtimes"), json!({"state":"missing"}));
    script(
        root.path(),
        "xcode-select",
        "[ \"$1\" = '-p' ] || exit 1\nprintf '%s\\n' \"$PATH\"",
    );
    assert_eq!(read(root.path(), "types"), json!({"state":"missing"}));
    script(
        root.path(),
        "xcrun",
        r#"[ "$1" = simctl ] && [ "$2" = list ] && [ "$4" = '-j' ] || exit 1
case "$3" in
runtimes) printf '%s' '{"runtimes":[{"identifier":"com.apple.CoreSimulator.SimRuntime.iOS-18-0","version":"18.0","isAvailable":true}]}' ;;
devicetypes) printf '%s' '{"devicetypes":[{"identifier":"com.apple.CoreSimulator.SimDeviceType.iPhone-16"}]}' ;;
*) exit 1 ;;
esac"#,
    );
    assert_eq!(
        read(root.path(), "runtimes")["entries"][0]["version"],
        "18.0"
    );
    assert_eq!(
        read(root.path(), "types")["entries"][0]["identifier"],
        "com.apple.CoreSimulator.SimDeviceType.iPhone-16"
    );
}
