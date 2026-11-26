#![cfg(target_os = "linux")]

#[path = "startup/control.rs"]
mod control;
#[path = "startup/terminal.rs"]
mod terminal;

use std::{
    fs,
    os::unix::{
        fs::{PermissionsExt as _, symlink},
        net::UnixListener,
    },
    path::Path,
    process::{Command, Output},
};

use serde_json::json;

struct Host {
    root: tempfile::TempDir,
    _socket: UnixListener,
}

impl Host {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in("/run")
            .unwrap();
        let dir = root.path();
        let socket = UnixListener::bind(dir.join("engine.sock")).unwrap();
        fs::set_permissions(dir.join("engine.sock"), fs::Permissions::from_mode(0o660)).unwrap();
        for folder in ["docker-config", "applications"] {
            fs::create_dir(dir.join(folder)).unwrap();
            fs::set_permissions(dir.join(folder), fs::Permissions::from_mode(0o700)).unwrap();
        }
        symlink("docker", dir.join("docker-current")).unwrap();
        let settings = json!({
            "version":1,
            "manageApplications":{
                "engine":"system", "socket":dir.join("engine.sock"),
                "docker":dir.join("docker-current"), "compose":dir.join("compose"),
                "acceptedUser":65534, "setOnAt":1
            },
            "allContainers":false, "openToNetwork":false, "hubMayDeleteVolumes":false,
            "adoptedProjects":[], "protected":[], "registriesWithLogin":[],
            "limits":{"memoryBytes":null,"processes":512,"processorBillionths":null,
                      "stopSeconds":30,"freeDiskBytes":10_737_418_240_u64},
            "labelPrefixes":[]
        });
        fs::write(dir.join("settings.json"), settings.to_string()).unwrap();
        Self {
            root,
            _socket: socket,
        }
    }

    fn probes(&self, docker: &str) {
        let script = format!("#!/bin/sh\n{docker}");
        executable(&self.root.path().join("docker"), &script);
        executable(
            &self.root.path().join("compose"),
            "#!/bin/sh\ntest \"$1 $2\" = 'version --short' || exit 1\nprintf 'v2.23.1\\n'\n",
        );
    }

    fn check(&self) -> Output {
        let dir = self.root.path();
        Command::new(env!("CARGO_BIN_EXE_anima-applications"))
            .arg("check")
            .arg(dir.join("settings.json"))
            .arg(dir.join("docker-config"))
            .arg(dir.join("applications"))
            .output()
            .unwrap()
    }

    fn expect_refusal(&self, reason: &str) {
        let result = self.check();
        assert!(!result.status.success(), "{result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(reason),
            "{result:?}"
        );
    }
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
#[ignore = "requires a root-owned fixture and the nobody account in a disposable Linux container"]
fn startup_refuses_untrusted_authority_before_running_the_version_probe() {
    let host = Host::new();
    let dir = host.root.path();
    host.probes(
        r#"test "$1" = '--host' && test "$3" = '--config' &&
test "$5 $6 $7" = 'version --format {{.Server.Version}}' &&
test "$DOCKER_HOST" = "$2" && test "$DOCKER_CONFIG" = "$4" || exit 1
printf 'called' > "${0%/*}/probed"
printf '28.0.0\n'
"#,
    );
    fs::set_permissions(dir.join("engine.sock"), fs::Permissions::from_mode(0o666)).unwrap();
    host.expect_refusal("accepted user of the daemon can connect");
    assert!(!dir.join("probed").exists());

    fs::set_permissions(dir.join("engine.sock"), fs::Permissions::from_mode(0o660)).unwrap();
    fs::write(
        dir.join("docker-config/config.json"),
        r#"{"credsStore":"synthetic"}"#,
    )
    .unwrap();
    host.expect_refusal("Docker configuration is invalid");
    assert!(!dir.join("probed").exists());

    fs::write(dir.join("docker-config/config.json"), "{}").unwrap();
    fs::set_permissions(dir.join("settings.json"), fs::Permissions::from_mode(0o666)).unwrap();
    host.expect_refusal("startup path cannot be trusted");
    assert!(!dir.join("probed").exists());

    fs::set_permissions(dir.join("settings.json"), fs::Permissions::from_mode(0o600)).unwrap();
    let accepted = host.check();
    assert!(accepted.status.success(), "{accepted:?}");
    assert!(dir.join("probed").is_file());
}

#[test]
#[ignore = "requires a root-owned fixture and the nobody account in a disposable Linux container"]
fn startup_checks_the_server_version_without_exposing_probe_output() {
    let host = Host::new();
    host.probes("printf '27.9.9\\n'\n");
    host.expect_refusal("engine is older than the lowest version");

    host.probes("printf 'private-synthetic-marker\\n'\n");
    let rejected = host.check();
    assert!(!rejected.status.success(), "{rejected:?}");
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("private-synthetic-marker"));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("version probe"));
}
