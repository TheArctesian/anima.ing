use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

pub struct Endpoint {
    pub socket: PathBuf,
    child: Child,
    _directory: TempDir,
}

impl Endpoint {
    pub fn start() -> Self {
        Self::spawn(false)
    }

    pub fn start_with_observation() -> Self {
        Self::spawn(true)
    }

    fn spawn(observe: bool) -> Self {
        let directory = tempfile::tempdir().expect("create private test runtime");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let socket = directory.path().join("control.sock");
        let mut program = Command::new(env!("CARGO_BIN_EXE_anima-osd"));
        program.args([
            "--socket",
            socket.to_str().unwrap(),
            "serve",
            "--systemctl",
            "/no-such-anima-test-systemctl",
        ]);
        if observe {
            program
                .arg("--console-snapshot")
                .arg(directory.path().join("console.json"));
        }
        let mut child = program
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !socket.exists() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "service exited before binding"
            );
            assert!(Instant::now() < deadline, "service did not bind");
            thread::sleep(Duration::from_millis(10));
        }
        Self {
            socket,
            child,
            _directory: directory,
        }
    }

    pub fn request(&self, bytes: &[u8]) -> serde_json::Value {
        let mut stream = UnixStream::connect(&self.socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        stream.write_all(bytes).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        serde_json::from_str(&reply).expect("one JSON response")
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.child.kill().expect("stop test service");
        self.child.wait().expect("reap test service");
    }
}
