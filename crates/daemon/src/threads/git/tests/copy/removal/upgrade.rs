//! Recovery across an upgrade after W1 saved a complete removal snapshot.

use agent_threads::git::{GitAction, GitOutcome};
use sha2::Digest as _;

use super::{Bench, PERSON, own_of};
use crate::threads::git::tests::THREAD;

#[cfg(unix)]
mod barriers;
#[cfg(unix)]
mod verification;

#[tokio::test]
async fn interrupted_w1_removal_preserves_its_complete_ref_with_partial_or_absent_tree() {
    for absent in [false, true] {
        let bench = Bench::new().await;
        let folder = bench.worktree().await;
        let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
        let original = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
        bench
            .git_in(&bench.project, &["update-ref", &kept, &original])
            .await;
        std::fs::write(own_of(&bench).join("anima-saved"), &original).unwrap();
        if absent {
            std::fs::remove_dir_all(&folder).unwrap();
        } else {
            std::fs::remove_file(folder.join("a.txt")).unwrap();
        }
        let obstruction = if absent {
            let receipt = own_of(&bench).with_extension("removed");
            std::fs::create_dir(&receipt).unwrap();
            receipt
        } else {
            let lock = bench.project.join(".git").join(format!("{kept}.lock"));
            std::fs::write(&lock, "another writer").unwrap();
            lock
        };
        let refused = bench.call(PERSON, GitAction::Remove).await;
        assert!(matches!(refused, GitOutcome::Refused { .. }), "{refused:?}");
        assert_eq!(
            bench.git_in(&bench.project, &["rev-parse", &kept]).await,
            original
        );
        assert!(own_of(&bench).join("anima-saved").is_file());
        if absent {
            std::fs::remove_dir(obstruction).unwrap();
        } else {
            std::fs::remove_file(obstruction).unwrap();
        }
        let outcome = bench.call(PERSON, GitAction::Remove).await;
        let GitOutcome::Removed {
            kept: Some(ref reported),
            ..
        } = outcome
        else {
            panic!("legacy recovery failed: {outcome:?}");
        };
        assert_eq!(reported, &kept);
        assert_eq!(
            bench.git_in(&bench.project, &["rev-parse", &kept]).await,
            original
        );
        assert_eq!(
            bench
                .git_in(&bench.project, &["show", &format!("{kept}:a.txt")])
                .await,
            "one"
        );
        assert_eq!(bench.call(PERSON, GitAction::Remove).await, outcome);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_snapshot_keeps_its_records_until_pack_and_lfs_directories_sync() {
    use crate::threads::git::place::walk::sync_probe;

    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
    let original = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
    bench
        .git_in(&bench.project, &["update-ref", &kept, &original])
        .await;
    bench.git_in(&bench.project, &["repack", "-ad"]).await;
    let own = own_of(&bench);
    std::fs::write(own.join("anima-saved"), &original).unwrap();
    let common = bench.project.join(".git");
    let content = b"fixture LFS content";
    let oid = crate::model::fetch::hex(&sha2::Sha256::digest(content));
    let lfs = format!("lfs/objects/{}/{}", &oid[..2], &oid[2..4]);
    for root in [&own, &common] {
        let leaf = root.join(&lfs);
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::write(leaf.join(&oid), content).unwrap();
    }
    let competing_lock = common.join(format!("{kept}.lock"));
    std::fs::write(&competing_lock, "another writer").unwrap();
    for dir in ["objects/pack", &lfs] {
        let fault = sync_probe::fail_for(&common.join(dir), || {});
        let removal = bench.call(PERSON, GitAction::Remove).await;
        drop(fault);
        assert!(
            matches!(removal, GitOutcome::Refused { why: agent_threads::git::GitRefusal::Failed { said } }
            if said.contains("injected directory sync failure")),
            "{dir}"
        );
        assert!(own.join("anima-saved").is_file());
        assert!(folder.is_dir());
        assert!(!own.with_extension("removed").exists());
    }
    std::fs::remove_file(competing_lock).unwrap();
    let removal = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(removal, GitOutcome::Removed { kept: Some(name), .. } if name == kept));
    assert_eq!(
        bench
            .git_in(&bench.project, &["show", &format!("{kept}:a.txt")])
            .await,
        "one"
    );
}

#[tokio::test]
async fn missing_legacy_pack_index_prevents_release() {
    missing_legacy_snapshot_data_prevents_release(true).await;
}

#[tokio::test]
async fn missing_legacy_lfs_payload_prevents_release() {
    missing_legacy_snapshot_data_prevents_release(false).await;
}

async fn missing_legacy_snapshot_data_prevents_release(pack_index: bool) {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let (kept, lfs) = saved_lfs_snapshot(&bench).await;
    let damaged = if pack_index {
        pack_index_at(&bench)
    } else {
        lfs
    };
    if !pack_index {
        std::fs::remove_dir_all(&folder).unwrap();
    }
    let removed = std::fs::read(&damaged).unwrap();
    std::fs::remove_file(&damaged).unwrap();
    assert_legacy_release_refused(&bench).await;
    assert_eq!(folder.is_dir(), pack_index);
    std::fs::write(damaged, removed).unwrap();
    assert_legacy_release_replayed(&bench, &kept).await;
}

async fn assert_legacy_release_refused(bench: &Bench) {
    let result = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(result, GitOutcome::Refused { .. }), "{result:?}");
    let own = own_of(bench);
    assert!(own.join("anima-saved").is_file());
    assert!(own.with_extension("json").is_file());
    assert!(!own.with_extension("removed").exists());
}

async fn assert_legacy_release_replayed(bench: &Bench, kept: &str) {
    let retry = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(&retry, GitOutcome::Removed { kept: Some(name), .. } if name == kept),
        "{retry:?}"
    );
    assert_eq!(bench.call(PERSON, GitAction::Remove).await, retry);
}

async fn saved_lfs_snapshot(bench: &Bench) -> (String, std::path::PathBuf) {
    let payload = b"saved LFS contents";
    let oid = crate::model::fetch::hex(&sha2::Sha256::digest(payload));
    let pointer = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
        payload.len()
    );
    std::fs::write(bench.project.join("pointer.bin"), pointer).unwrap();
    std::fs::write(
        bench.project.join(".gitattributes"),
        "pointer.bin filter=lfs\n",
    )
    .unwrap();
    bench
        .git_in(&bench.project, &["add", ".gitattributes", "pointer.bin"])
        .await;
    bench
        .git_in(&bench.project, &["commit", "-qm", "legacy snapshot"])
        .await;
    let original = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
    let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
    bench
        .git_in(&bench.project, &["update-ref", &kept, &original])
        .await;
    bench.git_in(&bench.project, &["repack", "-ad"]).await;
    let own = own_of(bench);
    std::fs::write(own.join("anima-saved"), &original).unwrap();
    let project = bench.project.join(".git");
    let leaf = project.join(format!("lfs/objects/{}/{}", &oid[..2], &oid[2..4]));
    std::fs::create_dir_all(&leaf).unwrap();
    let lfs = leaf.join(&oid);
    std::fs::write(&lfs, payload).unwrap();
    (kept, lfs)
}

fn pack_index_at(bench: &Bench) -> std::path::PathBuf {
    bench
        .project
        .join(".git/objects/pack")
        .read_dir()
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "idx"))
        .unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn an_archived_legacy_marker_keeps_its_ref_when_registry_release_is_retried() {
    use crate::threads::git::place::walk::sync_probe;
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let own = own_of(&bench);
    let active = own.with_extension("json");
    let mut without_id: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&active).unwrap()).unwrap();
    without_id.as_object_mut().unwrap().remove("folder_id");
    let without_id = serde_json::to_vec(&without_id).unwrap();
    std::fs::write(&active, &without_id).unwrap();

    let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
    let original = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
    bench
        .git_in(&bench.project, &["update-ref", &kept, &original])
        .await;
    std::fs::write(own.join("anima-saved"), &original).unwrap();
    std::fs::remove_dir_all(folder).unwrap();

    let parent = own.parent().unwrap();
    let original_git = own.clone();
    let fault = sync_probe::fail_when(parent, move || !original_git.exists());
    let interrupted = bench.call(PERSON, GitAction::Remove).await;
    drop(fault);
    assert!(
        matches!(interrupted, GitOutcome::Refused { why: agent_threads::git::GitRefusal::Failed { said } }
        if said.contains("injected directory sync failure"))
    );
    assert!(!own.exists());
    assert!(active.is_file());
    let archived = parent
        .read_dir()
        .unwrap()
        .flatten()
        .map(|entry| entry.path().join("anima-saved"))
        .find(|file| file.is_file())
        .expect("the marker was archived first");
    assert_eq!(std::fs::read_to_string(archived).unwrap(), original);

    let retry = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(&retry, GitOutcome::Removed { kept: Some(name), recovery: None, .. } if name == &kept),
        "{retry:?}"
    );
    assert_eq!(bench.call(PERSON, GitAction::Remove).await, retry);
    assert!(!active.exists());

    std::fs::write(active, without_id).unwrap();
    let new = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(new, GitOutcome::Removed { kept: None, .. }),
        "{new:?}"
    );
}
