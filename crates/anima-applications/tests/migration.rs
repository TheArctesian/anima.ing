#![cfg(target_os = "linux")]

#[path = "migration/compose.rs"]
mod compose;

use std::{
    fs,
    os::unix::{
        fs::{MetadataExt as _, PermissionsExt as _, chown, symlink},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};

use anima_applications::{Paths, Plan, Step, execution::Executor};

const APPLICATION: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e";
const IMPORT: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9f";
const BACKUP: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8da0";

#[test]
#[ignore = "requires root in a disposable Linux container"]
fn the_import_keeps_data_ownership_and_replays_after_the_prepared_directory_moves() {
    let host = Host::new();
    let state = host.staged().join("binds/state");
    let exported = state.join("record");
    let original = host.root.path().join("legacy-source");
    fs::write(&original, "kept by Podman").unwrap();
    fs::write(&exported, "independent backup").unwrap();
    fs::hard_link(&exported, state.join("hardlink")).unwrap();
    symlink("record", state.join("relative-link")).unwrap();
    chown(&state, Some(1200), Some(1300)).unwrap();
    chown(&exported, Some(1200), Some(1300)).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o750)).unwrap();

    let first = host.import();
    assert!(first.status.success(), "{first:?}");
    assert!(!host.staged().exists());
    let installed = host.application().join("binds/state");
    check_bind(&host, &installed);
    assert_eq!(fs::read_to_string(&original).unwrap(), "kept by Podman");
    fs::write(
        installed.join("record"),
        "Docker can update the imported copy",
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(installed.join("hardlink")).unwrap(),
        "Docker can update the imported copy"
    );
    assert_eq!(fs::read_to_string(original).unwrap(), "kept by Podman");

    let replay = host.import();
    assert!(replay.status.success(), "{replay:?}");
    assert_eq!(first.stdout, replay.stdout);
    let receipt: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(receipt["state"], "imported");
    assert_eq!(receipt["application"], APPLICATION);
    assert_eq!(receipt["backupId"], BACKUP);
    assert_eq!(receipt["binds"], serde_json::json!(["state"]));
}

fn check_bind(host: &Host, bind: &Path) {
    let root = host.root.path().canonicalize().unwrap();
    let socket = root.join("docker.sock");
    let _peer = UnixListener::bind(&socket).unwrap();
    let paths = Paths {
        docker: "/bin/false".to_owned(),
        compose: "/bin/false".to_owned(),
        socket: socket.to_str().unwrap().to_owned(),
        configuration: root.to_str().unwrap().to_owned(),
        root: root.to_str().unwrap().to_owned(),
    };
    let plan = Plan {
        steps: vec![Step::BindFolder {
            path: bind.to_str().unwrap().to_owned(),
        }],
    };
    let report = Executor::open(&paths)
        .unwrap()
        .run(&plan, Instant::now() + Duration::from_secs(1));
    assert_eq!(report.failed, None);
    let metadata = fs::metadata(bind).unwrap();
    assert_eq!(
        (metadata.uid(), metadata.gid(), metadata.mode() & 0o777),
        (1200, 1300, 0o750)
    );
    assert_eq!(
        fs::read_link(bind.join("relative-link")).unwrap(),
        Path::new("record")
    );
}

#[test]
#[ignore = "requires root in a disposable Linux container"]
fn the_import_refuses_data_linked_to_the_source_and_preserves_an_existing_target() {
    let host = Host::new();
    let outside = host.root.path().join("outside");
    fs::write(&outside, "legacy data").unwrap();
    let staged = host.staged().join("binds/state/record");
    fs::hard_link(&outside, &staged).unwrap();
    let linked = host.import();
    assert!(!linked.status.success(), "{linked:?}");
    assert!(host.staged().exists());
    assert!(!host.application().exists());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "legacy data");

    fs::remove_file(&staged).unwrap();
    fs::write(&staged, "independent backup").unwrap();
    private(&host.application());
    let existing = host.application().join("previous");
    fs::write(&existing, "existing application").unwrap();
    let conflict = host.import();
    assert!(!conflict.status.success(), "{conflict:?}");
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("already exists"));
    assert_eq!(
        fs::read_to_string(existing).unwrap(),
        "existing application"
    );
    assert_eq!(fs::read_to_string(staged).unwrap(), "independent backup");
}

struct Host {
    root: tempfile::TempDir,
}

impl Host {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let fixture = Self { root };
        for directory in [
            fixture.root.path().join("imports"),
            fixture.staged(),
            fixture.staged().join("binds"),
            fixture.staged().join("binds/state"),
        ] {
            private(&directory);
        }
        let record = fixture.staged().join("migration.json");
        let manifest = serde_json::json!({
            "schema": 1, "application": APPLICATION, "importId": IMPORT,
            "backupId": BACKUP, "podmanUid": 1000, "podmanContainers": ["b".repeat(64)],
            "binds": ["state"],
        });
        fs::write(&record, manifest.to_string()).unwrap();
        fs::set_permissions(record, fs::Permissions::from_mode(0o600)).unwrap();
        fixture
    }

    fn staged(&self) -> PathBuf {
        self.root.path().join("imports").join(IMPORT)
    }

    fn application(&self) -> PathBuf {
        self.root.path().join(APPLICATION)
    }

    fn import(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_anima-applications"))
            .arg("import-binds")
            .arg(self.root.path().canonicalize().unwrap())
            .args([APPLICATION, IMPORT])
            .output()
            .unwrap()
    }
}

fn private(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
