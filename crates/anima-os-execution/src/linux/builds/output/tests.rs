use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn stored_diagnostics_redact_across_chunks_and_omit_oversized_streams() {
    let work = std::env::temp_dir().join(format!("anima-build-output-{}", Uuid::now_v7()));
    fs::create_dir(&work).unwrap();
    let secret = "synthetic-private-token-12345678";
    let text = format!("{}\npassword={secret}\nFinished\n", "x".repeat(4080));
    let stdout = capture(text.as_bytes()).await.unwrap();
    let stderr = capture(
        b"-----BEGIN PRIVATE KEY-----\nsynthetic-key-body\n-----END PRIVATE KEY-----".as_slice(),
    )
    .await
    .unwrap();
    save(
        &work,
        &Output {
            schema: 1,
            program: "railpack".into(),
            stage: Stage::Detecting,
            finished: true,
            stdout,
            stderr,
        },
    )
    .unwrap();
    let path = work.join("output.json");
    let persisted = fs::read_to_string(&path).unwrap();
    assert!(!persisted.contains(secret));
    assert!(!persisted.contains("synthetic-key-body"));
    assert!(persisted.contains("Finished"));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let oversized = format!("useful diagnostic {}", "x".repeat(LIMIT));
    let output = capture(oversized.as_bytes()).await.unwrap();
    assert_eq!(
        output,
        "Output omitted: stream exceeded the diagnostic limit."
    );
    fs::remove_dir_all(work).unwrap();
}
