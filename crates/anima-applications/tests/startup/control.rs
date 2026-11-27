//! Exercise the real listener with disposable Linux users and a synthetic engine.
use std::{
    fs,
    os::unix::{fs::PermissionsExt as _, process::CommandExt as _},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::Host;

#[path = "compose.rs"]
mod compose;
#[path = "control/maintenance.rs"]
mod maintenance;

const DAEMON: u32 = 65_534;
const DEVICE: &str = "0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e";
const CONTAINER: &str = "a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4";
const HUB: &str = "https://first.example:443";
const OTHER: &str = "https://other.example:443";

#[test]
#[ignore = "requires root, the nobody account and Python in a disposable Linux container"]
fn control_authenticates_the_peer_preserves_the_pair_and_applies_the_container_policy() {
    let host = Host::new();
    let root = host.root.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o755)).unwrap();
    let socket_dir = root.join("socket");
    fs::create_dir(&socket_dir).unwrap();
    fs::set_permissions(&socket_dir, fs::Permissions::from_mode(0o755)).unwrap();
    host.probes(DOCKER);
    let settings_path = root.join("settings.json");
    let mut settings: Value = serde_json::from_slice(&fs::read(&settings_path).unwrap()).unwrap();
    settings["allContainers"] = json!(true);
    set(&settings_path, &settings);
    {
        let server = Server::start(root, &socket_dir);
        assert_eq!(server.probe(0, OTHER), json!({"outcome":"closed"}));
        assert_eq!(server.probe(DAEMON, HUB), json!({"outcome":"paired"}));
        exercise_actions(&server, root);
        assert_eq!(server.probe(DAEMON, OTHER), refused("applications_off"));
        assert_eq!(server.probe(DAEMON, HUB), refused("applications_off"));
    }
    {
        let server = Server::start(root, &socket_dir);
        assert_eq!(server.probe(DAEMON, HUB), refused("applications_off"));
        settings["manageApplications"]["setOnAt"] = json!(2);
        set(&settings_path, &settings);
        assert_eq!(server.probe(DAEMON, OTHER), refused("applications_off"));
    }
    let server = Server::start(root, &socket_dir);
    assert_eq!(server.probe(DAEMON, OTHER), json!({"outcome":"paired"}));
}

fn exercise_actions(server: &Server, root: &Path) {
    let start = server.send(DAEMON, HUB, &action("start"));
    assert_eq!(start, refused("start_not_permitted"));
    assert!(!root.join("stopped").exists());
    assert_eq!(
        server.request((DAEMON, HUB), &action("stop"), "abandon"),
        json!({"outcome":"ready"})
    );
    assert!(!root.join("stopped").exists());
    fs::write(root.join("running-once"), "").unwrap();
    assert_eq!(
        server.send(DAEMON, HUB, &action("restart")),
        json!({"outcome":"failed","cause":"step_failed"})
    );
    assert!(!root.join("restarted").exists());
    {
        use rustix::fs::{FlockOperation, flock};
        let active = fs::File::open(root.join("applications")).unwrap();
        flock(&active, FlockOperation::NonBlockingLockExclusive).unwrap();
        assert_eq!(
            server.send(DAEMON, HUB, &action("restart")),
            refused("busy")
        );
        assert_eq!(
            server.send(DAEMON, HUB, &action("stop")),
            json!({"outcome":"succeeded"})
        );
    }
    assert_eq!(fs::read_to_string(root.join("stopped")).unwrap(), CONTAINER);
}

fn set(path: &Path, value: &Value) {
    fs::write(path, value.to_string()).unwrap();
}

fn action(action: &str) -> Value {
    let item = json!({
        "id": DEVICE, "type":"container_action", "kind":"host", "attempt":1,
        "lease":{"until":"2026-11-25T00:00:00Z"}, "project":null, "secrets":[],
        "parameters":{"container":CONTAINER, "action":action},
        "limits":{"seconds":15,"idleSeconds":0,"memoryBytes":0,"diskBytes":0,"orphanSeconds":0}
    });
    json!({"type":"item", "json": item.to_string()})
}

fn refused(cause: &str) -> Value {
    json!({"outcome":"refused","cause":cause})
}

struct Server {
    process: Child,
    socket: PathBuf,
}

impl Server {
    fn start(root: &Path, socket_dir: &Path) -> Self {
        Self::start_at(root, socket_dir, &root.join("applications"))
    }

    fn start_at(root: &Path, socket_dir: &Path, applications: &Path) -> Self {
        let process = Command::new(env!("CARGO_BIN_EXE_anima-applications"))
            .arg("serve")
            .arg(root.join("settings.json"))
            .arg(root.join("docker-config"))
            .arg(applications)
            .arg(socket_dir)
            .env("ANIMA_LEGACY_BIN", root.join("legacy-bin"))
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let mut server = Self {
            process,
            socket: socket_dir.join("control.sock"),
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(server.process.try_wait().unwrap().is_none());
            if fs::symlink_metadata(&server.socket).is_ok_and(|metadata| {
                metadata.permissions().mode() & 0o777 == 0o660
                    && std::os::unix::net::UnixStream::connect(&server.socket).is_ok()
            }) {
                return server;
            }
            assert!(Instant::now() < deadline, "the control socket did not open");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn probe(&self, uid: u32, origin: &str) -> Value {
        self.send(uid, origin, &json!({"type":"probe"}))
    }

    fn send(&self, uid: u32, origin: &str, command: &Value) -> Value {
        self.request((uid, origin), command, "start")
    }

    fn request(&self, (uid, origin): (u32, &str), command: &Value, decision: &str) -> Value {
        self.exchange((uid, origin), command, (decision, &[]))
    }

    fn exchange(
        &self,
        (uid, origin): (u32, &str),
        command: &Value,
        (decision, files): (&str, &[&str]),
    ) -> Value {
        let message = json!({"origin":origin,"device":DEVICE,"command":command});
        let output = Command::new("python3")
            .args(["-c", CLIENT])
            .arg(&self.socket)
            .arg(message.to_string())
            .arg(decision)
            .args(files)
            .uid(uid)
            .gid(uid)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

const CLIENT: &str = r#"
import json, socket, struct, sys

def send(client, body):
    client.sendall(struct.pack(">I", len(body)) + body)

def read(reply):
    length = reply.read(4)
    return json.loads(reply.read(struct.unpack(">I", length)[0])) if length else {"outcome":"closed"}

with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
    client.settimeout(10)
    client.connect(sys.argv[1])
    try:
        send(client, sys.argv[2].encode())
        with client.makefile("rb") as reply:
            result = read(reply)
            if result["outcome"] == "needs_files" and len(sys.argv) > 4:
                for body in sys.argv[4:]:
                    send(client, body.encode())
                result = read(reply)
            if result["outcome"] == "ready" and sys.argv[3] == "start":
                send(client, b'{"type":"start"}')
                result = read(reply)
            if result["outcome"] == "checking_health":
                result = read(reply)
    except (ConnectionResetError, BrokenPipeError):
        result = {"outcome":"closed"}
print(json.dumps(result))
"#;

const DOCKER: &str = r#"
case "$5:$6" in
    version:--format) printf '28.0.0\n';;
    container:ls) printf '%s\n' "${11#id=}";;
    container:inspect)
        running=false
        if test -f "${0%/*}/running-once"; then
            /bin/rm "${0%/*}/running-once"
            running=true
        fi
        printf '{"id":"%s","name":"/demo","project":"","running":%s,"restart":"no"}\n' "${10}" "$running";;
    container:stop) test "$7" = -- && printf '%s' "$8" > "${0%/*}/stopped";;
    container:restart) printf '%s' "$8" > "${0%/*}/restarted";;
    *) exit 1;;
esac
"#;
