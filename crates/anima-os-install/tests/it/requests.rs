use serde_json::{Value, json};
use std::{
    io::Write as _,
    process::{Command, Stdio},
};

fn invoke(bytes: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_anima-os-install"))
        .args(["install", "--system", "/nix/store/synthetic-system"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("native installer");
    child
        .stdin
        .take()
        .expect("request input")
        .write_all(bytes)
        .expect("write request");
    child.wait_with_output().expect("installer response")
}

fn request() -> Value {
    json!({"system":"/nix/store/synthetic-system", "selected":{
        "path":"/dev/synthetic", "type":"disk", "size":68_719_476_736_u64, "maj:min":"254:0"},
        "confirmed":[], "hashed_password":"$y$syntheticSecret", "keymap":"keycode 1 = Escape\n"})
}

#[test]
fn missing_or_changed_authorization_never_reaches_disk_commands() {
    for confirmed in [
        json!([]),
        json!([{"path":"/dev/changed", "maj:min":"254:0", "size":68_719_476_736_u64}]),
    ] {
        let mut request = request();
        request["confirmed"] = confirmed;
        let result = invoke(&serde_json::to_vec(&request).expect("request"));
        assert!(!result.status.success());
        let output = String::from_utf8_lossy(&result.stdout);
        assert!(output.contains("authorization does not match"), "{output}");
        assert!(!output.contains("syntheticSecret"));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("syntheticSecret"));
    }
}

#[test]
fn bounded_invalid_requests_report_safe_errors() {
    for bytes in [
        b"{syntheticSecret".to_vec(),
        vec![b'x'; 2 * 1024 * 1024 + 1],
    ] {
        let result = invoke(&bytes);
        assert!(!result.status.success());
        let output: Value = serde_json::from_slice(&result.stdout).expect("safe JSON failure");
        assert_eq!(output["state"], "failed");
        assert!(!String::from_utf8_lossy(&result.stdout).contains("syntheticSecret"));
    }
}
