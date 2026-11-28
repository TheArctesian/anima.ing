use std::os::unix::{fs::symlink, process::ExitStatusExt as _};

use tokio::process::Command;

use super::*;

#[tokio::test]
async fn th32_cleanup_requires_an_empty_pass_after_signalling_marked_processes() {
    let runner = Uuid::now_v7();
    let wanted = format!("{MARK}={runner}");
    // Unrelated processes can make the real host's complete scan uncertain. Point the
    // test at its own entries while retaining real proc metadata, pidfds, and signals.
    let processes = tempfile::tempdir().unwrap();
    // A marked process can become visible after the preceding pass found its last match.
    for _ in 0..2 {
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .env(MARK, runner.to_string())
            .kill_on_drop(true)
            .spawn()
            .expect("the synthetic marked process starts");
        let pid = child.id().expect("the marked process PID");
        symlink(
            format!("/proc/{pid}"),
            processes.path().join(pid.to_string()),
        )
        .unwrap();
        let pass = tokio::time::timeout(
            Duration::from_secs(2),
            empty_after_sweep(processes.path(), &wanted),
        )
        .await
        .expect("the killing pass finishes");
        assert_eq!(
            pass,
            Some(false),
            "a killing pass does not certify emptiness"
        );
        let ended = child
            .wait()
            .await
            .expect("the marked process can be reaped");
        assert_eq!(ended.signal(), Some(Signal::KILL.as_raw()));
    }
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            empty_after_sweep(processes.path(), &wanted)
        )
        .await
        .expect("the confirming pass finishes"),
        Some(true)
    );
}
