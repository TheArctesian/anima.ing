//! The hub on the bundled PostgreSQL, given only two file names (OR-6, DB-002, R-016).

use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anima_database::{Layout, Limits, Role, Settings, Tree, startup};
use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{DEADLINE, Running, finished, spawn};

const SETTINGS: Settings = Settings {
    max_connections: 30,
    shared_buffers_mb: 16,
    flush_through_drive_cache: false,
};
const LIMITS: Limits = Limits {
    ready: Duration::from_secs(60),
    stop: Duration::from_secs(30),
};

/// Closed to the group whatever the umask: a backup refuses a folder others can write to.
fn private_folder() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        folder.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    folder
}

fn tree() -> Tree {
    let named = std::env::var_os("ANIMA_POSTGRES_TREE").map(PathBuf::from);
    let known = [
        "/opt/homebrew/opt/postgresql@17",
        "/usr/local/opt/postgresql@17",
        "/usr/lib/postgresql/17",
    ]
    .map(PathBuf::from);
    named
        .iter()
        .chain(&known)
        .find_map(|folder| Tree::at(folder).ok())
        .expect("no PostgreSQL 17 found: install it or set ANIMA_POSTGRES_TREE")
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn serve(envs: &[(&str, &str)]) -> (Running, SocketAddr) {
    let mut hub = spawn(&[], envs);
    let stdout = hub.0.stdout.take().unwrap();
    let (lines, received) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = lines.send(line);
        }
    });
    let started = Instant::now();
    loop {
        let left = DEADLINE.saturating_sub(started.elapsed());
        let line = received.recv_timeout(left).unwrap_or_else(|_| {
            let mut stderr = String::new();
            let _ = hub.0.stderr.take().unwrap().read_to_string(&mut stderr);
            panic!("the hub did not start: {stderr}");
        });
        if line.contains("hub started") {
            let line: serde_json::Value = serde_json::from_str(&line).unwrap();
            let listen = line["fields"]["listen"].as_str().unwrap().parse().unwrap();
            return (hub, listen);
        }
    }
}

fn migrate(owner_url: &Path, key: &Path) {
    let (migrated, stderr) = finished(spawn(
        &["migrate"],
        &[
            ("ANIMA_DATABASE_URL_FILE", text(owner_url)),
            ("ANIMA_KEY_FILE", text(key)),
        ],
    ));
    assert!(migrated, "{stderr}");
}

/// A hub that starts has passed its role checks on this database (R-016, LOG-006).
fn serves_and_is_healthy(hub_url: &Path, key: &Path) {
    let (hub, address) = serve(&[
        ("ANIMA_DATABASE_URL_FILE", text(hub_url)),
        ("ANIMA_KEY_FILE", text(key)),
        ("ANIMA_LISTEN", "127.0.0.1:0"),
    ]);
    let listen = address.to_string();
    let (healthy, stderr) = finished(spawn(&["health"], &[("ANIMA_LISTEN", &listen)]));
    assert!(healthy, "{stderr}");
    drop(hub);
}

/// A write through the hub's own code, as the hub login that owns nothing.
fn set_operator(hub_url: &Path, key: &Path) {
    let mut operator = super::spawn_with_input(
        &["operator", "set", "ada"],
        &[
            ("ANIMA_DATABASE_URL_FILE", text(hub_url)),
            ("ANIMA_KEY_FILE", text(key)),
        ],
    );
    operator
        .0
        .stdin
        .take()
        .unwrap()
        .write_all(b"a long test passphrase for ada\n")
        .unwrap();
    let (set, stderr) = finished(operator);
    assert!(set, "{stderr}");
}

#[test]
fn db_002_r_016_the_hub_migrates_and_serves_on_the_bundled_database() {
    let folder = private_folder();
    let layout = Layout::new(folder.path().join("db root"), folder.path().join("s")).unwrap();
    let database = startup::start(&tree(), &layout, SETTINGS, LIMITS).unwrap();
    let key = folder.path().join("anima_key");
    std::fs::write(&key, STANDARD.encode([5_u8; 32])).unwrap();

    migrate(&layout.url_file(Role::Owner), &key);
    set_operator(&layout.url_file(Role::Hub), &key);
    serves_and_is_healthy(&layout.url_file(Role::Hub), &key);

    // DB-007: a restored backup is a database the hub serves on.
    let backup = folder.path().join("anima.backup");
    anima_database::backup::write(&tree(), &layout, &backup).unwrap();
    assert_eq!(
        database.stop(LIMITS).unwrap(),
        anima_database::Stopped::Clean
    );
    let second = private_folder();
    let restored = Layout::new(second.path().join("db root"), second.path().join("s")).unwrap();
    let restored_database = startup::start(&tree(), &restored, SETTINGS, LIMITS).unwrap();
    anima_database::backup::restore(&tree(), &restored, &backup).unwrap();

    serves_and_is_healthy(&restored.url_file(Role::Hub), &key);
    migrate(&restored.url_file(Role::Owner), &key);
    restored_database.stop(LIMITS).unwrap();
}
