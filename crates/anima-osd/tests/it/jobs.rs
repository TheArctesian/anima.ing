use serde_json::{Value, json};
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

const REQUEST: &str = "cb882430-1c41-4b8c-b598-1ff4bc452a1f";

struct Appliance {
    root: TempDir,
    socket: PathBuf,
    journal: PathBuf,
    program: PathBuf,
    child: Option<Child>,
}

impl Appliance {
    fn new(slow: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let journal = root.path().join("journal");
        fs::create_dir(&journal).unwrap();
        fs::set_permissions(&journal, fs::Permissions::from_mode(0o700)).unwrap();
        let program = root.path().join("probe");
        let calls = root.path().join("calls");
        let sleep = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("sleep"))
            .find(|path| path.is_file())
            .expect("sleep on test PATH");
        let sleep = sleep.display().to_string().replace('\'', "'\\''");
        let ending = if slow {
            format!("exec '{sleep}' 5")
        } else {
            format!("'{sleep}' 0.15\nprintf 'active\\n'")
        };
        fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf 'probe\\n' >> '{}'\n{ending}\n",
                calls.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.path().join("control.sock");
        Self {
            root,
            socket,
            journal,
            program,
            child: None,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_anima-osd"));
        command
            .arg("--socket")
            .arg(&self.socket)
            .arg("serve")
            .arg("--systemctl")
            .arg(&self.program)
            .arg("--state-directory")
            .arg(&self.journal)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    fn start(&mut self) {
        let mut child = self.command().spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.socket.exists() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "service exited before listening"
            );
            assert!(Instant::now() < deadline, "service did not listen");
            thread::sleep(Duration::from_millis(10));
        }
        self.child = Some(child);
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().unwrap();
            child.wait().unwrap();
            fs::remove_file(&self.socket).unwrap();
        }
    }

    fn stream(&self, request: &Value) -> UnixStream {
        let mut stream = UnixStream::connect(&self.socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        writeln!(stream, "{request}").unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        stream
    }

    fn request(&self, request: &Value) -> Value {
        let mut stream = self.stream(request);
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn get(&self) -> Value {
        self.request(&json!({"version":2,"operation":"operation_get","request_id":REQUEST}))
    }

    fn wait_state(&self, state: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let reply = self.get();
            if reply["value"]["state"] == state {
                return reply["value"].clone();
            }
            assert!(Instant::now() < deadline, "expected {state}, got {reply}");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_probes(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.probe_count() < 2 {
            assert!(Instant::now() < deadline, "probes did not start");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn probe_count(&self) -> usize {
        fs::read_to_string(self.root.path().join("calls"))
            .unwrap_or_default()
            .lines()
            .count()
    }
}

impl Drop for Appliance {
    fn drop(&mut self) {
        self.stop();
    }
}

fn start() -> Value {
    json!({"version":2,"operation":"operation_start","request_id":REQUEST,"action":"diagnostics"})
}

#[test]
fn lost_acceptance_and_restart_recover_the_same_completed_operation() {
    let mut appliance = Appliance::new(false);
    appliance.start();
    drop(appliance.stream(&start()));
    let completed = appliance.wait_state("succeeded");
    assert_eq!(completed["result"]["monitoring"], "active");
    assert_eq!(completed["result"]["private_link"], "active");
    assert_eq!(appliance.request(&start())["value"], completed);
    assert_eq!(
        appliance.probe_count(),
        2,
        "retry must not execute probes again"
    );
    appliance.stop();
    appliance.start();
    assert_eq!(appliance.get()["value"], completed);
    assert_eq!(appliance.request(&start())["value"], completed);
    let list = appliance.request(&json!({"version":2,"operation":"operation_list","after":null}));
    assert_eq!(list["value"]["records"], json!([completed]));
    assert_eq!(appliance.probe_count(), 2);
}

#[test]
fn cancellation_finishes_cleanup_while_status_remains_available() {
    let mut appliance = Appliance::new(true);
    appliance.start();
    let accepted = appliance.request(&start());
    assert_eq!(accepted["result"], "operation");
    appliance.wait_probes();
    let status = appliance.request(&json!({"version":1,"operation":"status"}));
    assert_eq!(status["result"], "status");
    assert_eq!(appliance.get()["value"]["state"], "running");
    let cancel =
        json!({"version":2,"operation":"operation_cancel","operation_id":accepted["value"]["id"]});
    let pending = appliance.request(&cancel);
    assert_eq!(pending["value"]["cancel_requested"], true);
    let finished = appliance.wait_state("cancelled");
    assert!(finished["result"].is_null());
    assert!(
        finished["revision"].as_u64().unwrap() > accepted["value"]["revision"].as_u64().unwrap()
    );
    assert_eq!(appliance.request(&cancel)["value"], finished);
    assert_eq!(appliance.request(&start())["value"], finished);
    assert_eq!(appliance.probe_count(), 2);
}

#[test]
fn restart_interrupts_inflight_work_instead_of_replaying_it() {
    let mut appliance = Appliance::new(true);
    appliance.start();
    let accepted = appliance.request(&start());
    appliance.wait_probes();
    assert_eq!(appliance.get()["value"]["state"], "running");
    appliance.stop();
    appliance.start();
    let interrupted = appliance.wait_state("interrupted");
    assert_eq!(interrupted["id"], accepted["value"]["id"]);
    assert!(interrupted["result"].is_null());
    assert_eq!(appliance.request(&start())["value"], interrupted);
    assert_eq!(
        appliance.probe_count(),
        2,
        "restart must not run probes again"
    );
}

#[test]
fn journal_faults_refuse_jobs_without_stopping_status() {
    let mut appliance = Appliance::new(false);
    appliance.start();
    let mut second = Appliance::new(false);
    second.journal.clone_from(&appliance.journal);
    second.start();
    assert_eq!(second.request(&start())["value"], "journal_unavailable");
    assert_eq!(
        second.request(&json!({"version":1,"operation":"status"}))["result"],
        "status"
    );
    assert_eq!(appliance.request(&start())["result"], "operation");
    appliance.wait_state("succeeded");
    appliance.stop();
    let corrupt = appliance.journal.join("corrupt.json");
    fs::write(&corrupt, "not-json").unwrap();
    fs::set_permissions(&corrupt, fs::Permissions::from_mode(0o600)).unwrap();
    appliance.start();
    assert_eq!(appliance.request(&start())["value"], "journal_unavailable");
    assert_eq!(
        appliance.request(&json!({"version":1,"operation":"status"}))["result"],
        "status"
    );
    assert_eq!(fs::read_to_string(corrupt).unwrap(), "not-json");
}

#[test]
fn malformed_job_requests_never_start_a_probe() {
    let mut appliance = Appliance::new(false);
    appliance.start();
    let mut extra = start();
    extra["command"] = json!("anything");
    let mut mutation = start();
    mutation["action"] = json!("reboot");
    let mut invalid_id = start();
    invalid_id["request_id"] = json!("not-a-uuid");
    for request in [extra, mutation, invalid_id] {
        let reply = appliance.request(&request);
        assert_eq!(reply["result"], "error");
        assert_eq!(reply["value"], "invalid_request");
    }
    let mut duplicate = UnixStream::connect(&appliance.socket).unwrap();
    duplicate
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    duplicate.write_all(b"{\"version\":2,\"operation\":\"operation_list\",\"operation\":\"operation_list\",\"after\":null}\n").unwrap();
    duplicate.shutdown(std::net::Shutdown::Write).unwrap();
    let mut bytes = Vec::new();
    duplicate.read_to_end(&mut bytes).unwrap();
    let reply: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reply["result"], "error");
    assert_eq!(reply["value"], "invalid_request");
    let list = appliance.request(&json!({"version":2,"operation":"operation_list","after":null}));
    assert_eq!(list["value"]["records"], json!([]));
    assert_eq!(appliance.probe_count(), 0);
    assert_eq!(appliance.request(&start())["result"], "operation");
    appliance.wait_state("succeeded");
    assert_eq!(appliance.probe_count(), 2);
}

#[test]
fn operation_pages_have_a_stable_cursor_without_duplicates() {
    let mut appliance = Appliance::new(false);
    appliance.start();
    let mut ids = Vec::new();
    for _ in 0..33 {
        let request_id = uuid::Uuid::now_v7().to_string();
        let accepted = appliance.request(&json!({"version":2,"operation":"operation_start","request_id":request_id,"action":"diagnostics"}));
        assert_eq!(accepted["result"], "operation");
        ids.push(accepted["value"]["id"].clone());
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let reply = appliance
                .request(&json!({"version":2,"operation":"operation_get","request_id":request_id}));
            if reply["value"]["state"] == "succeeded" {
                break;
            }
            assert!(Instant::now() < deadline, "job did not finish: {reply}");
            thread::sleep(Duration::from_millis(10));
        }
    }
    let first = appliance.request(&json!({"version":2,"operation":"operation_list","after":null}));
    assert_eq!(first["value"]["records"].as_array().unwrap().len(), 32);
    let next = &first["value"]["next"];
    assert_eq!(next, &ids[31]);
    let last = appliance.request(&json!({"version":2,"operation":"operation_list","after":next}));
    let records = last["value"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], ids[32]);
    assert!(last["value"]["next"].is_null());
}
