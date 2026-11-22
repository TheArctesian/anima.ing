use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt as _, process::Command};

fn inventory(disks: &[Value]) -> std::process::Output {
    let directory = tempfile::tempdir().expect("fixture directory");
    let script = directory.path().join("lsblk");
    let path = std::env::var("PATH").expect("executable search path");
    let shell = std::env::split_paths(&path)
        .map(|dir| dir.join("sh"))
        .find(|file| file.is_file())
        .expect("POSIX shell");
    let data = serde_json::to_string(&json!({"blockdevices": disks})).expect("inventory JSON");
    fs::write(
        &script,
        format!(
            "#!{}\ncat <<'INVENTORY'\n{data}\nINVENTORY\n",
            shell.display()
        ),
    )
    .expect("fixture executable");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).expect("fixture permissions");
    Command::new(env!("CARGO_BIN_EXE_anima-os-install"))
        .arg("inventory")
        .env("PATH", format!("{}:{path}", directory.path().display()))
        .output()
        .expect("native inventory")
}

#[test]
fn eligibility_excludes_every_live_or_removable_dependency() {
    let disk = json!({"path":"/dev/synthetic", "type":"disk", "size":68_719_476_736_u64,
        "maj:min":"254:0", "ro":false, "rm":false, "mountpoints":[null]});
    let mut disks = vec![disk.clone()];
    for (key, value) in [
        ("ro", json!(true)),
        ("rm", json!(true)),
        ("tran", json!("usb")),
        ("size", json!(1024)),
        ("mountpoints", json!(["/mounted"])),
        (
            "children",
            json!([{"path":"/dev/synthetic1", "type":"part", "mountpoints":["/mounted"]}]),
        ),
        (
            "children",
            json!([{"path":"/dev/mapper/synthetic", "type":"crypt"}]),
        ),
    ] {
        let mut excluded = disk.clone();
        excluded[key] = value;
        disks.push(excluded);
    }
    let result = inventory(&disks);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let values: Vec<Value> = serde_json::from_slice(&result.stdout).expect("inventory response");
    assert_eq!(values[0]["eligible"], true);
    assert!(values[1..].iter().all(|disk| disk["eligible"] == false));
}

#[test]
fn flat_partition_inventory_fails_closed() {
    let result = inventory(&[json!({"path":"/dev/synthetic1", "type":"part"})]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("dependency tree"));
}
