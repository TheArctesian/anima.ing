use super::{Identity, ROOT, capture, command, record};
use anima_os_maintenance::{Admission, drain, recovery, source_pipeline::Pipeline};
use std::{fs, os::unix::fs::DirBuilderExt, path::Path};
use uuid::Uuid;

pub(in crate::linux) const PRESSURE: &str = "/run/anima-hosting-pressure";

#[tokio::test]
#[ignore = "requires disposable root Linux with an empty Btrfs /persist and packaged commands"]
async fn capture_respects_holds_without_revoking_the_accepted_pipeline() {
    let identity = prepare_workspace().await;
    let execution = Uuid::now_v7();
    let admission = Admission::cleanup().unwrap();
    recovery::complete(&admission).unwrap();
    clear_sample();
    Pipeline::accept(&admission, (execution, identity.workspace, Uuid::now_v7())).unwrap();
    drop(admission);
    let pipeline = Pipeline::resume(execution).unwrap();
    drain::begin().unwrap();

    let refused = capture(identity, None).await.unwrap_err().to_string();
    assert!(refused.contains("draining work"), "{refused}");
    assert!(!Path::new(ROOT).exists());
    let wrong_workspace = Identity {
        workspace: Uuid::now_v7(),
        ..identity
    };
    let refused = capture(wrong_workspace, Some(pipeline))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("does not match the admitted pipeline"),
        "{refused}"
    );

    let hold = Path::new(PRESSURE).join("active");
    fs::write(&hold, b"synthetic hosting pressure").unwrap();
    let refused = capture(identity, Some(pipeline))
        .await
        .unwrap_err()
        .to_string();
    assert!(refused.contains("memory pressure"), "{refused}");
    assert!(!Path::new(ROOT).exists());
    fs::remove_file(hold).unwrap();

    clear_sample();
    let captured = capture(identity, Some(pipeline)).await.unwrap();
    assert_eq!(captured["phase"], "captured");
    assert_eq!(captured["entries"], 1);
    let exported = Path::new(ROOT)
        .join(identity.context.to_string())
        .join("source/example");
    assert_eq!(fs::read(exported).unwrap(), b"test fixture");
    fs::write(
        Path::new(PRESSURE).join("active"),
        b"synthetic hosting pressure",
    )
    .unwrap();
    assert_eq!(capture(identity, None).await.unwrap(), captured);
}

pub(in crate::linux) async fn prepare_workspace() -> Identity {
    for path in ["/run/anima-os", PRESSURE, "/persist/anima-workspaces"] {
        fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    }
    fs::create_dir("/persist/anima-workspaces/homes").unwrap();
    fs::create_dir("/persist/anima-workspaces/locks").unwrap();
    let workspace = Uuid::now_v7();
    let home = record::home(workspace);
    command::run("btrfs", &["subvolume", "create", home.to_str().unwrap()])
        .await
        .unwrap();
    fs::create_dir(home.join("workspace")).unwrap();
    fs::write(home.join("workspace/example"), b"test fixture").unwrap();
    let mut entry = record::Record::new(workspace, 1 << 30);
    entry.generation = 1;
    entry.phase = record::Phase::Stopped;
    record::save(&entry).unwrap();
    Identity {
        workspace,
        generation: 1,
        context: Uuid::now_v7(),
    }
}

pub(in crate::linux) fn clear_sample() {
    let uptime = fs::read_to_string("/proc/uptime").unwrap();
    let (seconds, _) = uptime.split_once('.').unwrap();
    fs::write(Path::new(PRESSURE).join("clear"), seconds).unwrap();
}
