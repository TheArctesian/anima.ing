//! `anima-link` needs a relay, so each test runs a stand-in that prints its ready line.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

mod platform;
mod root_copy;

const LIMIT: Duration = Duration::from_secs(120);
fn local_hub() -> PathBuf {
    std::env::var_os("ANIMA_LOCAL_HUB_EXE").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_anima-local-hub")),
        PathBuf::from,
    )
}

#[cfg(unix)]
const LINK: &str = r#"#!/bin/sh
trap 'exit 0' TERM
echo 'time=2026-11-15T10:00:00Z level=INFO msg="the hub link is ready" relay=stand-in devices=0' >&2
while :; do sleep 0.1; done
"#;

fn built() -> PathBuf {
    let folder = local_hub().parent().unwrap().to_path_buf();
    for name in ["anima-hub", "anima-database", "anima-gateway"] {
        assert!(
            platform::program(&folder, name).is_file(),
            "{name} is not built beside anima-local-hub: build the workspace (`cargo build --workspace`)"
        );
    }
    folder
}

fn programs(rig: &Rig) -> PathBuf {
    platform::programs(rig.folder.path(), &built())
}

fn tree() -> PathBuf {
    let named = std::env::var_os("ANIMA_POSTGRES_TREE").map(PathBuf::from);
    let known = [
        "/opt/homebrew/opt/postgresql@17",
        "/usr/local/opt/postgresql@17",
        "/usr/lib/postgresql/17",
    ]
    .map(PathBuf::from);
    named
        .into_iter()
        .chain(known)
        .find(|folder| anima_database::Tree::at(folder).is_ok())
        .expect("no PostgreSQL 17 found: install it or set ANIMA_POSTGRES_TREE")
}

struct Rig {
    folder: TempDir,
}

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Rig {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        anima_windows::files::protect(folder.path()).unwrap();
        // Closed to the group whatever the umask: a backup refuses a group-writable folder.
        #[cfg(unix)]
        std::fs::set_permissions(
            folder.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        Self { folder }
    }

    fn root(&self) -> PathBuf {
        self.folder.path().join("hub root")
    }

    fn command(&self, command: &str) -> Command {
        let mut program = Command::new(local_hub());
        platform::environment(&mut program);
        program
            .env("HOME", self.folder.path())
            .arg(command)
            .arg("--root")
            .arg(self.root())
            .arg("--socket-folder")
            .arg(self.folder.path().join("s"));
        program
    }

    fn run(&self) -> Running {
        let mut child = self
            .command("run")
            .args(["--listen", "127.0.0.1:0", "--programs"])
            .arg(programs(self))
            .arg("--tree")
            .arg(tree())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut running = Running(child);
        let (lines, received) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = lines.send(line);
            }
        });
        match received.recv_timeout(LIMIT) {
            Ok(line) if line == "ready" => running,
            other => {
                let ended = running.0.try_wait();
                panic!(
                    "the hub did not get ready: {other:?}, {ended:?}, {}",
                    self.logs()
                );
            }
        }
    }

    fn status(&self) -> serde_json::Value {
        let output = self.command("status").output().unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn logs(&self) -> String {
        [
            "local-hub.log",
            "anima-database.log",
            "anima-hub.log",
            "anima-gateway.log",
            "anima-link.log",
        ]
        .map(|name| {
            let text = std::fs::read_to_string(self.root().join("log").join(name));
            format!("--- {name}\n{}", text.unwrap_or_default())
        })
        .join("\n")
    }
}

fn stop(mut running: Running) -> ExitStatus {
    platform::stop(&mut running.0);
    let started = Instant::now();
    loop {
        if let Some(status) = running.0.try_wait().unwrap() {
            return status;
        }
        assert!(started.elapsed() < LIMIT, "the hub did not stop");
        thread::sleep(Duration::from_millis(50));
    }
}

fn hub_address(status: &serde_json::Value) -> SocketAddr {
    status["hub"].as_str().unwrap().parse().unwrap()
}

fn hub_is_healthy(address: SocketAddr) -> bool {
    let mut command = Command::new(platform::program(&built(), "anima-hub"));
    platform::environment(&mut command);
    command
        .arg("health")
        .env("ANIMA_LISTEN", address.to_string())
        .status()
        .unwrap()
        .success()
}

fn status_line_of(address: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(LIMIT)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    answer.lines().next().unwrap_or_default().to_owned()
}

fn assert_all_stopped(rig: &Rig, address: SocketAddr) {
    let status = rig.status();
    assert_eq!(status["running"], false, "{status}");
    assert_eq!(status["state"], "stopped", "{status}");
    assert!(!hub_is_healthy(address));
    let socket = rig.folder.path().join("s/.s.PGSQL.5432");
    assert!(!socket.exists(), "the database still runs");
    let database_log = std::fs::read_to_string(rig.root().join("database/server.log")).unwrap();
    assert!(
        database_log.contains("database system is shut down"),
        "{database_log}"
    );
}

#[test]
fn dbl_012_the_real_group_serves_stops_in_order_and_starts_again_on_the_same_database() {
    let rig = Rig::new();
    assert_eq!(rig.status()["running"], false);

    let running = rig.run();
    let status = rig.status();
    assert_eq!(status["state"], "ready", "{status}");
    assert_eq!(status["recovered"], false, "{status}");
    let address = hub_address(&status);
    assert!(address.ip().is_loopback());
    assert!(hub_is_healthy(address), "{}", rig.logs());
    let key = std::fs::read(rig.root().join("vault.key")).unwrap();

    assert!(stop(running).success(), "{}", rig.logs());
    assert_all_stopped(&rig, address);

    let again = rig.run();
    assert!(hub_is_healthy(hub_address(&rig.status())), "{}", rig.logs());
    assert_eq!(std::fs::read(rig.root().join("vault.key")).unwrap(), key);
    assert!(stop(again).success());
}

#[test]
fn f_6_the_hub_is_in_the_link_mode_and_its_link_is_up() {
    let rig = Rig::new();
    let running = rig.run();
    let address = hub_address(&rig.status());

    // NET-001: in the link mode this computer is a permitted client.
    assert_eq!(status_line_of(address, "/health"), "HTTP/1.1 200 OK");
    let graphql = status_line_of(address, "/graphql");
    assert!(
        graphql.starts_with("HTTP/1.1 ") && !graphql.contains("403"),
        "{graphql}"
    );
    platform::assert_socket(&rig.folder.path().join("s/link.sock"));
    let started = Instant::now();
    while rig.status()["link"]["state"] != "up" {
        assert!(started.elapsed() < LIMIT, "{}", rig.logs());
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        rig.status()["link"]["since"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );

    assert!(stop(running).success());
}

#[test]
fn frn_007_account_refuses_with_code_20_while_the_hub_does_not_run_and_checks_the_name() {
    let rig = Rig::new();

    let not_ready = rig.command("account").arg("status").output().unwrap();
    let bad_name = rig
        .command("account")
        .args(["create", "ada/x"])
        .output()
        .unwrap();

    assert_eq!(not_ready.status.code(), Some(20), "{not_ready:?}");
    assert_eq!(not_ready.stdout, [] as [u8; 0]);
    assert_eq!(bad_name.status.code(), Some(2), "{bad_name:?}");
}

#[test]
fn a_second_run_of_the_same_folder_ends_with_code_10() {
    let rig = Rig::new();
    let running = rig.run();

    let second = rig
        .command("run")
        .arg("--programs")
        .arg(programs(&rig))
        .arg("--tree")
        .arg(tree())
        .output()
        .unwrap();

    assert_eq!(second.status.code(), Some(10), "{second:?}");
    assert_eq!(rig.status()["state"], "ready");
    assert!(stop(running).success());
}

#[test]
fn update_of_a_hub_that_is_not_installed_prints_one_json_line_and_changes_nothing() {
    let rig = Rig::new();

    let output = rig
        .command("update")
        .arg("--tree")
        .arg(rig.folder.path().join("no tree"))
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "{\"updated\":false}\n"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(!rig.root().exists(), "the hub root was made");
}
