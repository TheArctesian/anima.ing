use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt as _};

use super::*;

#[tokio::test]
async fn cmd_010_open_waits_for_windows_sharing_contention() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("auth.lock");
    private_file::write_new(dir.path(), "auth.lock", b"").expect("a private lock file");
    let exclusive = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("an exclusive open handle");
    assert_eq!(
        open_file(&path)
            .expect_err("an incompatible share mode")
            .raw_os_error(),
        Some(ERROR_SHARING_VIOLATION)
    );

    let deadline = Instant::now() + Duration::from_millis(125);
    let error = tokio::time::timeout(Duration::from_secs(5), open_lock(&path, deadline))
        .await
        .expect("open stops retrying at its deadline")
        .expect_err("the exclusive handle is still open");
    assert!(Instant::now() >= deadline);
    assert_eq!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION));

    let mut opening = std::pin::pin!(open_lock(&path, Instant::now() + WAIT_LIMIT));
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut opening)
            .await
            .is_err(),
        "open must remain pending while the other handle excludes it"
    );
    drop(exclusive);
    let file = tokio::time::timeout(Duration::from_secs(5), opening)
        .await
        .expect("open resumes when the other handle closes")
        .expect("a validated lock handle");
    file.try_lock()
        .expect("the validated handle supports locking");
}

#[tokio::test]
async fn cmd_010_open_reports_a_terminal_error_without_waiting_for_contention() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        open_lock(dir.path(), Instant::now() + WAIT_LIMIT),
    )
    .await
    .expect("a terminal error is returned promptly")
    .expect_err("a directory cannot be a credential lock file");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}
