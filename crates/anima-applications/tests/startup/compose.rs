//! Drive a managed apply through the real socket and inspect what the programs received.
#[path = "compose/cutover.rs"]
mod cutover;

use std::{
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::Path,
};

use super::{DAEMON, DEVICE, HUB, Host, Server, fs, json, set};
use crate::executable;

const PROJECT: &str = "anima-0192f6b07c1e7a3b9d2f4e5a6b7c8d9e";
const COMPOSE: &str =
    "services:\n  demo:\n    image: library/alpine:3.22\n    environment:\n      DEMO: ${DEMO}\n";
const ENVIRONMENT: &str = "DEMO=synthetic\n";
const DIGEST: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn apply_pins_the_successful_pull_and_reuses_only_its_own_digest() {
    let (host, socket) = fixture();
    let root = host.root.path();
    let server = Server::start(root, &socket);
    assert_eq!(
        server.send(DAEMON, HUB, &json!({"type":"probe_apply"})),
        json!({"outcome":"apply_available"})
    );
    assert_blocks_unsafe_start(&server, root);

    let result = server.exchange(
        (DAEMON, HUB),
        &apply(true),
        ("start", &[COMPOSE, ENVIRONMENT]),
    );
    assert_eq!(result, json!({"outcome":"applied","health":"healthy"}));
    assert_installed(root);

    fs::remove_file(root.join("pulled")).unwrap();
    let reused = server.exchange(
        (DAEMON, HUB),
        &apply(false),
        ("start", &[COMPOSE, ENVIRONMENT]),
    );
    assert_eq!(reused, json!({"outcome":"applied","health":"healthy"}));
    assert!(!root.join("pulled").exists());
    assert_eq!(
        fs::read_to_string(root.join("started")).unwrap(),
        format!("{PROJECT}\n{PROJECT}\n")
    );

    let files = installed_files(root);
    fs::remove_file(files.join("environment-of-the-application")).unwrap();
    assert_eq!(
        deploy(&server, false, UPDATED),
        json!({"outcome":"failed","cause":"no_digest"})
    );
    assert_eq!(deploy(&server, true, UPDATED), applied());
    assert!(!files.join("kept-digests.json").exists());

    fs::write(root.join("foreign-project"), "").unwrap();
    assert_eq!(
        server.send(DAEMON, HUB, &apply(true)),
        super::refused("name_taken")
    );
}

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn a_failed_start_preserves_the_previous_final_form_and_version() {
    let (host, socket) = fixture();
    let root = host.root.path();
    let server = Server::start(root, &socket);
    assert_eq!(deploy(&server, true, ENVIRONMENT), applied());
    assert_eq!(deploy(&server, false, UPDATED), applied());
    let files = installed_files(root);
    let original = fs::read(files.join("final-form.json")).unwrap();
    fs::write(root.join("fail-compose"), "").unwrap();

    assert_eq!(
        deploy(&server, false, ENVIRONMENT),
        json!({"outcome":"failed","cause":"step_failed"})
    );
    assert!(fs::read(files.join("final-form.json")).unwrap() == original);
    assert_eq!(
        fs::read_to_string(files.join("environment-of-the-application")).unwrap(),
        UPDATED
    );
    assert_eq!(
        fs::read_to_string(files.join("kept-environment-of-the-application")).unwrap(),
        ENVIRONMENT
    );
}

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn restart_finishes_confirmed_version_records_without_starting_compose_again() {
    let (host, socket) = fixture();
    let root = host.root.path();
    let server = Server::start(root, &socket);
    assert_eq!(deploy(&server, true, ENVIRONMENT), applied());
    let files = installed_files(root);
    let kept_environment = files.join("kept-environment-of-the-application");
    fs::create_dir(&kept_environment).unwrap();
    assert_eq!(
        deploy(&server, false, UPDATED),
        json!({"outcome":"failed","cause":"orphan"})
    );
    drop(server);
    fs::remove_dir(&kept_environment).unwrap();

    let server = Server::start(root, &socket);
    assert_eq!(
        server.request((DAEMON, HUB), &apply(false), "abandon"),
        json!({"outcome":"needs_files"})
    );
    assert_eq!(
        fs::read_to_string(root.join("started")).unwrap(),
        format!("{PROJECT}\n{PROJECT}\n")
    );
    assert_eq!(fs::read_to_string(&kept_environment).unwrap(), ENVIRONMENT);
    assert_eq!(
        fs::read_to_string(files.join("environment-of-the-application")).unwrap(),
        UPDATED
    );
    assert_eq!(
        fs::read_to_string(files.join("compose-as-it-came.yaml")).unwrap(),
        UPDATED_COMPOSE
    );
    assert_eq!(
        fs::read_to_string(files.join("kept-compose-as-it-came.yaml")).unwrap(),
        COMPOSE
    );
    let form: serde_json::Value =
        serde_json::from_slice(&fs::read(files.join("final-form.json")).unwrap()).unwrap();
    assert_eq!(form["services"]["demo"]["environment"]["DEMO"], "updated");
}

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn losing_the_owner_before_compose_exit_is_recorded_restores_the_confirmed_version() {
    let (host, socket) = fixture();
    let root = host.root.path();
    let server = Server::start(root, &socket);
    assert_eq!(deploy(&server, true, ENVIRONMENT), applied());
    let files = installed_files(root);
    let original = fs::read(files.join("final-form.json")).unwrap();

    fs::write(root.join("kill-owner"), "").unwrap();
    assert_eq!(deploy(&server, false, UPDATED), json!({"outcome":"closed"}));
    drop(server);
    fs::remove_file(root.join("kill-owner")).unwrap();
    assert!(fs::read(files.join("final-form.json")).unwrap() != original);

    let server = Server::start(root, &socket);
    assert_eq!(
        server.request((DAEMON, HUB), &apply(false), "abandon"),
        json!({"outcome":"needs_files"})
    );
    assert!(fs::read(files.join("final-form.json")).unwrap() == original);
    assert_eq!(
        fs::read_to_string(files.join("environment-of-the-application")).unwrap(),
        ENVIRONMENT
    );
    assert_eq!(
        fs::read_to_string(root.join("started")).unwrap(),
        format!("{PROJECT}\n{PROJECT}\n")
    );
}

const UPDATED: &str = "DEMO=updated\n";
const UPDATED_COMPOSE: &str = concat!(
    "services:\n  demo:\n    image: library/alpine:3.22\n",
    "    environment:\n      DEMO: ${DEMO}\n# second version\n"
);

fn applied() -> serde_json::Value {
    json!({"outcome":"applied","health":"healthy"})
}

fn deploy(server: &Server, pull: bool, environment: &str) -> serde_json::Value {
    let mut request = apply(pull);
    let compose = if environment == UPDATED {
        let mut item: serde_json::Value =
            serde_json::from_str(request["json"].as_str().unwrap()).unwrap();
        item["parameters"]["environmentSha256"] =
            json!("f1bca8d78e3e0bf6c93e1ff327a791d43729f401627e012ce52b87494ad198c4");
        item["parameters"]["fileSha256"] =
            json!("dc784b7d335de32c5eaeaa73f74d3b52267ca67175dd074747e341e9189079b1");
        request["json"] = json!(item.to_string());
        UPDATED_COMPOSE
    } else {
        COMPOSE
    };
    server.exchange((DAEMON, HUB), &request, ("start", &[compose, environment]))
}

fn fixture() -> (Host, std::path::PathBuf) {
    let host = Host::new();
    let root = host.root.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap();
    let socket = root.join("socket");
    fs::create_dir(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o755)).unwrap();
    host.probes(DOCKER);
    executable(&root.join("compose"), COMPOSE_PROGRAM);
    let path = root.join("settings.json");
    let mut settings: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    settings["limits"]["freeDiskBytes"] = json!(1);
    set(&path, &settings);

    (host, socket)
}

fn assert_blocks_unsafe_start(server: &Server, root: &Path) {
    let staged = server.exchange(
        (DAEMON, HUB),
        &apply(true),
        ("abandon", &[COMPOSE, ENVIRONMENT]),
    );
    assert_eq!(staged, json!({"outcome":"ready"}));
    let modified = server.exchange(
        (DAEMON, HUB),
        &apply(true),
        ("start", &[COMPOSE, "DEMO=changed\n"]),
    );
    assert_eq!(
        modified,
        json!({"outcome":"failed","cause":"program_mismatch"})
    );
    assert!(!root.join("pulled").exists());
    assert!(!root.join("started").exists());

    fs::write(root.join("reject-digest"), "").unwrap();
    let unverified = server.exchange(
        (DAEMON, HUB),
        &apply(true),
        ("start", &[COMPOSE, ENVIRONMENT]),
    );
    assert_eq!(unverified, json!({"outcome":"failed","cause":"no_digest"}));
    assert!(!root.join("started").exists());
    fs::remove_file(root.join("reject-digest")).unwrap();
}

fn installed_files(root: &Path) -> std::path::PathBuf {
    root.join("applications").join(DEVICE).join("files")
}

fn assert_installed(root: &Path) {
    let folder = installed_files(root);
    let final_form = folder.join("final-form.json");
    let form: serde_json::Value = serde_json::from_slice(&fs::read(&final_form).unwrap()).unwrap();
    assert_eq!(
        form["services"]["demo"]["image"],
        format!("library/alpine@{DIGEST}")
    );
    assert_eq!(form["services"]["demo"]["environment"]["DEMO"], "synthetic");
    assert_eq!(fs::metadata(final_form).unwrap().mode() & 0o777, 0o600);
}

fn apply(pull: bool) -> serde_json::Value {
    let item = json!({
        "id": DEVICE, "type":"application_apply", "kind":"host", "attempt":1,
        "lease":{"until":"2026-11-25T00:00:00Z"}, "project":null,
        "secrets":["compose_file","environment_file"],
        "parameters":{"application":DEVICE, "fileSha256":"beb16db1afc4c8b737e87a297a1937d5d5aaef873e87517b6515bdae265054cd",
                      "environmentSha256":"0de5c2aa976170292c3d55a58a2c2bf24ddeee3af043b3843d0fe4158904e476", "pull":pull, "keep":1},
        "limits":{"seconds":15,"idleSeconds":0,"memoryBytes":0,"diskBytes":0,"orphanSeconds":0}
    });
    json!({"type":"item", "json": item.to_string()})
}

const DOCKER: &str = include_str!("compose/engine.sh");

const COMPOSE_PROGRAM: &str = r#"#!/bin/sh
if test "$1 $2" = 'version --short'; then printf 'v2.23.1\n'; exit; fi
root="${0%/*}"
project="$2"
test "$DOCKER_HOST" = "unix://$root/engine.sock" && test "$DOCKER_CONFIG" = "$root/docker-config" || exit 1
test "$1 $3 $5" = '--project-name --project-directory --file' || exit 1
test "$6" = "$4/final-form.json" || exit 1
shift 6
if test "$*" = 'config --hash=*'; then
    printf 'demo cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\n'
    exit
fi
test "$*" = 'up -d --remove-orphans --pull never --no-build' || exit 1
test ! -f "$root/fail-compose" || exit 1
printf '%s\n' "$project" >> "$root/started"
if test -f "$root/kill-owner"; then kill -KILL "$PPID"; fi
"#;
