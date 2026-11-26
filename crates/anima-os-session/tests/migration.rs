#![cfg(target_os = "linux")]

use std::{
    fs,
    io::{self, Write as _},
    os::unix::fs::{PermissionsExt as _, symlink},
    path::Path,
    process::{Command, Stdio},
};

use serde_json::{Value, json};

const ROOTS: [&str; 3] = ["/run/current-system", "/var/lib/containers", "/nonexistent"];
const TOOLS: &str = "/run/current-system/sw/bin";
const NOBODY: u32 = 65_534;

#[test]
#[ignore = "requires root and unused appliance paths in a disposable Linux container"]
fn migration_reports_startup_evidence_in_the_original_account_scope() {
    let _fixture = Appliance::new();
    let system = inventory(0);
    assert_eq!(
        system["startup"]["quadlets"],
        json!([
            {"name":"web.container","path":"/etc/containers/systemd/web.container","unit":"custom-web.service"}
        ])
    );
    assert_eq!(
        system["startup"]["units"],
        json!([
            {"name":"container-archive.service","enablement":"disabled","evidence":"name_pattern"},
            {"name":"custom-web.service","enablement":"generated","evidence":"quadlet"},
            {"name":"archive.service","enablement":"enabled","evidence":"container_label"}
        ])
    );
    assert_eq!(
        system["containers"][0]["systemdUnitHint"],
        "archive.service"
    );

    let account = inventory(NOBODY);
    assert_eq!(system["containers"][0]["autoRemove"], false);
    assert_eq!(account["runtimeContext"]["rootless"], true);
    assert_eq!(account["containers"][0]["autoRemove"], true);
    assert!(account["containers"][0]["systemdUnitHint"].is_null());
    assert_eq!(account["startup"]["units"], json!([]));
    assert!(account["startup"]["quadlets"].is_null());
    assert!(!account.to_string().contains("synthetic-private-error"));
}

fn inventory(uid: u32) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_anima-os-session"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let payload = json!({"id":"inventory","op":"migration-inventory","args":{"uid":uid}});
    writeln!(child.stdin.take().unwrap(), "{payload}").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = std::str::from_utf8(&output.stdout).unwrap();
    let response = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|response| response["id"] == "inventory")
        .unwrap();
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["uid"], uid);
    response["value"].clone()
}

struct Appliance;

impl Appliance {
    fn new() -> Self {
        for path in ROOTS {
            assert_eq!(
                fs::symlink_metadata(path).unwrap_err().kind(),
                io::ErrorKind::NotFound
            );
        }
        let fixture = Self;
        for path in [
            TOOLS,
            "/var/lib/containers/storage",
            "/nonexistent/.local/share/containers/storage",
        ] {
            fs::create_dir_all(path).unwrap();
        }
        symlink("/usr/bin/env", format!("{TOOLS}/env")).unwrap();
        symlink("/usr/sbin/runuser", format!("{TOOLS}/runuser")).unwrap();
        setup_tool("podman", PODMAN);
        setup_tool("systemctl", SYSTEMCTL);
        fixture
    }
}

impl Drop for Appliance {
    fn drop(&mut self) {
        for path in ROOTS {
            if Path::new(path).exists() {
                fs::remove_dir_all(path).unwrap();
            }
        }
    }
}

fn setup_tool(name: &str, script: &str) {
    let path = format!("{TOOLS}/{name}");
    fs::write(&path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

const PODMAN: &str = r#"#!/bin/sh
test "$1" = --remote=false || exit 1
rootless=false
hint=archive.service
if test "$(/usr/bin/id -u)" = 65534; then
    test "$HOME" = /nonexistent && test "$XDG_RUNTIME_DIR" = /run/user/65534 || exit 1
    rootless=true
    hint=../../hidden.service
fi
case "$2:$3" in
    ps:--all) printf '%064d\n' 1;;
    container:inspect)
        case "$5" in *autoRemove*) ;; *) exit 1;; esac
        printf '{"name":"fixture","mounts":[],"systemdUnitHint":"%s","autoRemove":%s}\n' "$hint" "$rootless";;
    volume:inspect|network:ls) :;;
    info:--format) printf '{"rootless":%s}\n' "$rootless";;
    quadlet:list)
        if $rootless; then echo synthetic-private-error >&2; exit 1; fi
        printf '{"name":"web.container","path":"/etc/containers/systemd/web.container","unit":"custom-web.service"}\n';;
    *) exit 1;;
esac
"#;

const SYSTEMCTL: &str = r#"#!/bin/sh
test "$7" = list-unit-files || exit 1
case "$1:$(/usr/bin/id -u)" in
    --system:0) printf 'container-archive.service disabled disabled\ncustom-web.service generated -\narchive.service enabled disabled\nnetwork.service enabled enabled\n';;
    --user:65534) :;;
    *) exit 1;;
esac
"#;
