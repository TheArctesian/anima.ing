use super::endpoint::Endpoint;
use std::{
    fs,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::Path,
    thread,
    time::{Duration, Instant},
};

#[test]
fn the_read_only_service_refreshes_a_bounded_snapshot_without_a_console_request() {
    let endpoint = Endpoint::start_with_observation();
    let path = endpoint.socket.with_file_name("console.json");
    let first = wait_for_update(&path, None);
    assert_eq!(first.permissions().mode() & 0o777, 0o640);
    let snapshot: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(snapshot["schema"], 1);
    assert!(snapshot["memoryTotal"].as_u64().unwrap() > 0);
    assert!(snapshot["uptime"].is_u64());
    assert!(snapshot["batteries"].is_array() || snapshot["batteries"].is_null());
    assert!(snapshot["cpu"].is_null());
    assert!(first.len() <= 16_384);
    let updated = wait_for_update(&path, Some(first.ino()));
    assert_ne!(updated.ino(), first.ino());
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).is_ok());
}

fn wait_for_update(path: &Path, previous: Option<u64>) -> fs::Metadata {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Ok(file) = fs::metadata(path)
            && previous != Some(file.ino())
        {
            return file;
        }
        assert!(
            Instant::now() < deadline,
            "the observer did not publish a snapshot"
        );
        thread::sleep(Duration::from_millis(25));
    }
}
