//! Exercise the installed owner and its maintenance boundary in a disposable container.
use std::{os::unix::net::UnixListener, process::Output};

use anima_os_maintenance::{Lease, Operation, cutover::APPLICATION_ROOT, hosting::SYSTEM_SOCKET};
use tokio::runtime::Runtime;

use super::*;
use crate::executable;

#[test]
#[ignore = "requires root and unused appliance paths in a disposable Linux container"]
fn a_power_lease_and_managed_docker_dispatch_exclude_each_other() {
    let installed = Installed::new();
    let host = Host::new();
    let root = host.root.path();
    let server = server(&host);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    power_wins(&runtime, root, &server);
    docker_wins(&runtime, root, &server);
    assert_eq!(
        fs::read_to_string(root.join("restarts")).unwrap(),
        "restart\n"
    );
    drop(server);
    drop(installed);
}

fn power_wins(runtime: &Runtime, root: &Path, server: &Server) {
    let hold = root.join("hold-inspection");
    fs::write(&hold, "").unwrap();
    thread::scope(|scope| {
        let request = scope.spawn(|| server.send(DAEMON, HUB, &action("restart")));
        appeared(&root.join("inspected"));
        let lease = runtime
            .block_on(Lease::prepare(Operation::Restart))
            .unwrap();
        fs::remove_file(hold).unwrap();
        assert_eq!(
            request.join().unwrap(),
            json!({"outcome":"failed","cause":"busy"})
        );
        assert!(!root.join("restarts").exists());
        assert_eq!(
            server.send(DAEMON, HUB, &json!({"type":"probe_apply"})),
            refused("busy")
        );
        assert_eq!(server.send(DAEMON, HUB, &action("start")), refused("busy"));
        assert_eq!(
            server.send(DAEMON, HUB, &action("stop")),
            json!({"outcome":"succeeded"})
        );
        assert!(root.join("stopped").is_file());
        drop(lease);
    });
}

fn docker_wins(runtime: &Runtime, root: &Path, server: &Server) {
    let hold = root.join("hold-restart");
    fs::write(&hold, "").unwrap();
    thread::scope(|scope| {
        let request = scope.spawn(|| server.send(DAEMON, HUB, &action("restart")));
        appeared(&root.join("restarts"));
        let power = runtime.block_on(Lease::prepare(Operation::Shutdown));
        assert!(power.is_err());
        fs::write(root.join("interrupt-restart"), "").unwrap();
        fs::remove_file(hold).unwrap();
        assert_eq!(
            request.join().unwrap(),
            json!({"outcome":"failed","cause":"orphan"})
        );
    });

    let power = runtime.block_on(Lease::prepare(Operation::Restart));
    assert!(matches!(power, Err(error) if error.to_string().contains("needs reconciliation")));
    let receipt = cli(&["dispatch-status"]);
    let receipt: Value = serde_json::from_slice(&receipt.stdout).unwrap();
    let id = receipt["id"].as_str().unwrap();
    assert_eq!(receipt["operation"], "restart");
    assert!(
        !cli(&["release-dispatch", DEVICE, "--verified-no-pending-command"])
            .status
            .success()
    );
    assert!(
        cli(&["release-dispatch", id, "--verified-no-pending-command"])
            .status
            .success()
    );
    assert!(runtime.block_on(Lease::prepare(Operation::Restart)).is_ok());
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_anima-applications"))
        .args(args)
        .output()
        .unwrap()
}

fn appeared(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "the fixture did not reach {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn server(host: &Host) -> Server {
    let root = host.root.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap();
    let socket = root.join("socket");
    fs::create_dir(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o755)).unwrap();
    host.probes(DOCKER);
    let path = root.join("settings.json");
    let mut settings: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    settings["allContainers"] = json!(true);
    settings["manageApplications"]["socket"] = json!(SYSTEM_SOCKET);
    set(&path, &settings);
    Server::start_at(root, &socket, Path::new(APPLICATION_ROOT))
}

const FIXTURE_DIRECTORIES: [&str; 3] = ["/persist", "/run/anima-os", "/run/current-system"];

struct Installed(UnixListener);

impl Installed {
    fn new() -> Self {
        for path in FIXTURE_DIRECTORIES.into_iter().chain([SYSTEM_SOCKET]) {
            assert_eq!(
                fs::symlink_metadata(path).unwrap_err().kind(),
                std::io::ErrorKind::NotFound,
                "fixture path is in use: {path}"
            );
        }
        for path in FIXTURE_DIRECTORIES {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::create_dir(APPLICATION_ROOT).unwrap();
        fs::set_permissions(APPLICATION_ROOT, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir_all("/persist/anima-admission/reservations").unwrap();
        fs::create_dir_all("/run/current-system/sw/bin").unwrap();
        executable(
            Path::new("/run/current-system/sw/bin/docker"),
            "#!/bin/sh\ntest \"$5 $6\" = 'ps --quiet'\n",
        );
        let socket = UnixListener::bind(SYSTEM_SOCKET).unwrap();
        fs::set_permissions(SYSTEM_SOCKET, fs::Permissions::from_mode(0o660)).unwrap();
        Self(socket)
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        if self.0.local_addr().is_ok() {
            fs::remove_file(SYSTEM_SOCKET).unwrap();
        }
        for path in FIXTURE_DIRECTORIES {
            fs::remove_dir_all(path).unwrap();
        }
    }
}

const DOCKER: &str = r#"
root="${0%/*}"
wait_for_release() {
    until test ! -e "$1"; do /bin/sleep 0.02; done
}
case "$5:$6" in
    version:--format) printf '28.0.0\n';;
    container:ls) printf '%s\n' "${11#id=}";;
    container:inspect)
        if test -f "$root/hold-inspection"; then
            : > "$root/inspected"
            wait_for_release "$root/hold-inspection"
        fi
        printf '{"id":"%s","name":"/demo","project":"","running":false,"restart":"always"}\n' "${10}";;
    container:stop) : > "$root/stopped";;
    container:restart)
        printf 'restart\n' >> "$root/restarts"
        wait_for_release "$root/hold-restart"
        if test -f "$root/interrupt-restart"; then kill -KILL $$; fi;;
    *) exit 1;;
esac
"#;
