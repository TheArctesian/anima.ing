use super::endpoint::Endpoint;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

const STATUS: &[u8] = b"{\"version\":1,\"operation\":\"status\"}\n";

#[test]
fn status_and_diagnostic_client_read_the_real_endpoint() {
    let endpoint = Endpoint::start();
    let reply = endpoint.request(STATUS);
    assert_eq!(reply["version"], 1);
    assert_eq!(reply["result"], "status");
    assert_eq!(
        reply["value"]["hostname"],
        fs::read_to_string("/proc/sys/kernel/hostname")
            .unwrap()
            .trim()
    );
    assert!(reply["value"]["memory_total_bytes"].as_u64().unwrap() > 0);
    assert!(reply["value"]["uptime_seconds"].is_u64());
    assert_eq!(
        fs::metadata(&endpoint.socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let output = Command::new(env!("CARGO_BIN_EXE_anima-osd"))
        .args(["--socket", endpoint.socket.to_str().unwrap(), "diagnostics"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(diagnostics["result"], "diagnostics");
    assert_eq!(diagnostics["value"]["monitoring"], "unknown");
    assert_eq!(diagnostics["value"]["private_link"], "unknown");
}

#[test]
fn invalid_requests_cannot_dispatch_or_stop_later_reads() {
    let endpoint = Endpoint::start();
    for (bytes, failure) in [
        (b"not-json\n".as_slice(), "invalid_request"),
        (
            b"{\"version\":2,\"operation\":\"status\"}\n".as_slice(),
            "unsupported_version",
        ),
        (
            b"{\"version\":1,\"operation\":\"reboot\"}\n".as_slice(),
            "invalid_request",
        ),
        (
            b"{\"version\":1,\"operation\":\"status\",\"command\":\"anything\"}\n".as_slice(),
            "invalid_request",
        ),
        (
            b"{\"version\":1,\"operation\":\"status\"}".as_slice(),
            "invalid_request",
        ),
    ] {
        let reply = endpoint.request(bytes);
        assert_eq!(reply["result"], "error");
        assert_eq!(reply["value"], failure);
    }
    let oversized = vec![b'x'; 1025];
    assert_eq!(endpoint.request(&oversized)["value"], "request_too_large");
    assert_eq!(endpoint.request(STATUS)["result"], "status");
}

#[test]
fn startup_refuses_shared_directories_and_preserves_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("control.sock");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let start = || {
        Command::new(env!("CARGO_BIN_EXE_anima-osd"))
            .args(["--socket", socket.to_str().unwrap(), "serve"])
            .output()
            .unwrap()
    };
    assert!(!start().status.success());
    assert!(!socket.exists());
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&socket, "owner file must survive").unwrap();
    assert!(!start().status.success());
    assert_eq!(
        fs::read_to_string(&socket).unwrap(),
        "owner file must survive"
    );
}

#[test]
fn a_stalled_client_expires_without_blocking_other_reads() {
    use std::{io::Read, os::unix::net::UnixStream, time::Duration};
    let endpoint = Endpoint::start();
    let mut idle = UnixStream::connect(&endpoint.socket).unwrap();
    idle.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    assert_eq!(endpoint.request(STATUS)["result"], "status");
    let mut response = String::new();
    idle.read_to_string(&mut response).unwrap();
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["value"], "timed_out");
    assert_eq!(endpoint.request(STATUS)["result"], "status");
}

#[test]
fn client_refuses_a_successful_reply_for_the_wrong_operation() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
    };
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.path().join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let client = std::thread::spawn(move || {
        Command::new(env!("CARGO_BIN_EXE_anima-osd"))
            .args(["--socket", socket.to_str().unwrap(), "status"])
            .output()
            .unwrap()
    });
    let (mut stream, _) = listener.accept().unwrap();
    let mut request = String::new();
    BufReader::new(&mut stream).read_line(&mut request).unwrap();
    stream.write_all(b"{\"version\":1,\"result\":\"diagnostics\",\"value\":{\"service_root_read_only\":null,\"monitoring\":\"unknown\",\"private_link\":\"unknown\"}}\n").unwrap();
    drop(stream);
    let output = client.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.stdout, [] as [u8; 0]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not match"));
}
