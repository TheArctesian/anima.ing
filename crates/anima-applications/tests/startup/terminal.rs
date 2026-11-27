use super::{Command, Host, Output, Path, executable, fs, json};
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};

const APPLICATION: &str = "0190a000-0000-7000-8002-000000000003";
const PROJECT: &str = "anima-0190a000000070008002000000000003";

#[test]
#[ignore = "requires a root-owned fixture and the nobody account in a disposable Linux container"]
fn console_target_requires_a_current_exact_unprivileged_application_container() {
    let host = Host::new();
    let fixture = host.root.path();
    let container = "a".repeat(64);
    let digest = format!("sha256:{}", "b".repeat(64));
    let files = install(fixture, &digest);
    host.probes(
        r#"
case "$5 $6 $7" in
    'version --format {{.Server.Version}}') printf '28.0.0\n' ;;
    'volume inspect --format') printf '{"o":"bind","device":"/"}\n' ;;
    'container inspect --format') cat "${0%/*}/inspection.json" ;;
    *) exit 1 ;;
esac
"#,
    );
    let hash = "e".repeat(64);
    executable(
        &fixture.join("compose"),
        &format!(
            "#!/bin/sh\ncase \"$1 $2\" in 'version --short') printf 'v2.23.1\\n';; *) printf 'web {hash}\\n';; esac\n"
        ),
    );
    let valid = json!({
        "id":container,"name":"/example","running":true,"workspace":false,
        "project":PROJECT,"service":"web","image":format!("example/web@{digest}"),
        "user":"1010:1011","folder":files,"final_form":files.join("final-form.json"),
        "config_hash":hash,"isolated":true,"network":"bridge","ipc":"private","security":["no-new-privileges"],
        "mounts":[]
    });
    let resolve = |inspection: &serde_json::Value| {
        fs::write(fixture.join("inspection.json"), inspection.to_string()).unwrap();
        command(fixture, &container)
    };
    let accepted = resolve(&valid);
    assert!(accepted.status.success(), "{accepted:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&accepted.stdout).unwrap(),
        json!({"container":container,"name":"example","application":APPLICATION,
               "service":"web","user":"1010:1011"})
    );
    let mut root_image = valid.clone();
    root_image["user"] = json!("0:0");
    let fallback = resolve(&root_image);
    assert!(fallback.status.success(), "{fallback:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fallback.stdout).unwrap()["user"],
        "65534:65534"
    );
    refusals(&valid, resolve);
}

fn refusals(valid: &serde_json::Value, resolve: impl Fn(&serde_json::Value) -> Output) {
    let cases = [
        ("name", json!("/do-not-open"), "application is protected"),
        ("id", json!("c".repeat(64)), "running, locally managed"),
        ("workspace", json!(true), "running, locally managed"),
        (
            "project",
            serde_json::Value::Null,
            "running, locally managed",
        ),
        ("running", json!(false), "running, locally managed"),
        ("isolated", json!(false), "shares host privileges"),
        (
            "network",
            json!("container:arbitrary"),
            "shares host privileges",
        ),
        (
            "image",
            json!("different/image:latest"),
            "differs from the installed",
        ),
        ("service", json!("worker"), "differs from the installed"),
        (
            "config_hash",
            json!("f".repeat(64)),
            "differs from the installed",
        ),
        (
            "mounts",
            json!([{"Type":"volume","Name":format!("{PROJECT}_data"),"Driver":"local"}]),
            "shares host privileges",
        ),
        (
            "mounts",
            json!([{"Type":"bind","Source":"/run/docker.sock"}]),
            "shares host privileges",
        ),
    ];
    for (field, replacement, reason) in cases {
        let mut denied = valid.clone();
        denied[field] = replacement;
        let reply = resolve(&denied);
        assert!(!reply.status.success(), "{field}: {reply:?}");
        assert!(
            String::from_utf8_lossy(&reply.stderr).contains(reason),
            "{field}: {reply:?}"
        );
        assert!(reply.stdout.is_empty(), "{field}: {reply:?}");
    }
}

fn install(root: &Path, digest: &str) -> std::path::PathBuf {
    let path = root.join("settings.json");
    let mut settings: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    settings["protected"] = json!(["do-not-open"]);
    fs::write(&path, settings.to_string()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let files = root.join("applications").join(APPLICATION).join("files");
    fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(&files)
        .unwrap();
    for name in [
        "compose-as-it-came.yaml",
        "environment-of-the-application",
        "final-form.json",
    ] {
        fs::write(files.join(name), "{}").unwrap();
        fs::set_permissions(files.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    fs::write(
        files.join("digests.json"),
        json!([
            {"service":"web","image":"example/web:latest","digest":digest}
        ])
        .to_string(),
    )
    .unwrap();
    fs::set_permissions(
        files.join("digests.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    files
}

fn command(root: &Path, container: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_anima-applications"))
        .arg("console-target")
        .arg(root.join("settings.json"))
        .arg(root.join("docker-config"))
        .arg(root.join("applications"))
        .arg(container)
        .output()
        .unwrap()
}
