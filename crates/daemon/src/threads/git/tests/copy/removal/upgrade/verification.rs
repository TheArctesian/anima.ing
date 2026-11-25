use std::path::{Path, PathBuf};

use super::*;

mod cache;
mod scratch;

async fn saved(bench: &Bench) -> (String, String) {
    bench.git_in(&bench.project, &["add", "."]).await;
    bench
        .git_in(
            &bench.project,
            &["commit", "--allow-empty", "-qm", "legacy checkpoint"],
        )
        .await;
    let commit = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
    let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
    bench
        .git_in(&bench.project, &["update-ref", &kept, &commit])
        .await;
    std::fs::write(own_of(bench).join("anima-saved"), &commit).unwrap();
    (kept, commit)
}

fn loose(objects: &Path, oid: &str) -> PathBuf {
    objects.join(&oid[..2]).join(&oid[2..])
}

#[tokio::test]
async fn truncated_symlink_blob_preserves_legacy_registration_until_restored() {
    let bench = Bench::new().await;
    bench.worktree().await;
    std::os::unix::fs::symlink(
        "lengthy-fixture-symlink-target-which-must-be-inflated",
        bench.project.join("link"),
    )
    .unwrap();
    let (kept, commit) = saved(&bench).await;
    let oid = bench
        .git_in(&bench.project, &["rev-parse", &format!("{commit}:link")])
        .await;
    let blob = loose(&bench.project.join(".git/objects"), &oid);
    let content = std::fs::read(&blob).unwrap();
    assert!(content.len() > 20);
    std::fs::remove_file(&blob).unwrap();
    std::fs::write(&blob, &content[..content.len() - 6]).unwrap();
    assert_legacy_release_refused(&bench).await;
    std::fs::write(blob, content).unwrap();
    assert_legacy_release_replayed(&bench, &kept).await;
}

#[tokio::test]
async fn a_snapshot_cannot_depend_on_the_object_store_being_archived() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let (kept, commit) = saved(&bench).await;
    let project = bench.project.join(".git/objects");
    let private = own_of(&bench).join("objects");
    let original = loose(&project, &commit);
    let alternate = loose(&private, &commit);
    std::fs::create_dir_all(alternate.parent().unwrap()).unwrap();
    std::fs::rename(&original, &alternate).unwrap();
    let config = project.join("info/alternates");
    std::fs::write(&config, format!("{}\n", private.display())).unwrap();
    assert_legacy_release_refused(&bench).await;
    assert!(alternate.is_file());
    std::fs::rename(alternate, original).unwrap();
    std::fs::remove_file(config).unwrap();
    assert_legacy_release_replayed(&bench, &kept).await;
}

async fn pointer(bench: &Bench, tracked: bool, variant: &str) -> (String, PathBuf) {
    let payload = b"verified payload";
    let oid = crate::model::fetch::hex(&sha2::Sha256::digest(payload));
    let version = "version https://git-lfs.github.com/spec/v1";
    let fields = format!("oid sha256:{oid}\nsize {}\n", payload.len());
    let text = match variant {
        "blank" => format!(" \n\n{version}\r\n\r\n{fields}\n"),
        "1024" => format!("{version}\n{fields}")
            .chars()
            .chain(std::iter::repeat(' '))
            .take(1024)
            .collect(),
        _ => format!("{version}\n{fields}"),
    };
    std::fs::write(bench.project.join("example.bin"), text).unwrap();
    if tracked {
        std::fs::write(
            bench.project.join(".gitattributes"),
            "example.bin filter=lfs\n",
        )
        .unwrap();
    }
    let (kept, _) = saved(bench).await;
    let leaf = bench
        .project
        .join(format!(".git/lfs/objects/{}/{}", &oid[..2], &oid[2..4]));
    std::fs::create_dir_all(&leaf).unwrap();
    (kept, leaf.join(oid))
}

#[tokio::test]
async fn accepts_pointer_shaped_text_without_a_saved_lfs_filter() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let (kept, _) = pointer(&bench, false, "plain").await;
    // The current worktree attributes are irrelevant to the saved checkout.
    std::fs::write(
        bench.project.join(".gitattributes"),
        "example.bin filter=lfs\n",
    )
    .unwrap();
    assert_legacy_release_replayed(&bench, &kept).await;
}

async fn payload_is_required_for_variant(variant: &str) {
    let bench = Bench::new().await;
    bench.worktree().await;
    let (kept, payload) = pointer(&bench, true, variant).await;
    // Even if current attributes no longer select LFS, the saved tree still does.
    std::fs::write(
        bench.project.join(".gitattributes"),
        "example.bin -filter\n",
    )
    .unwrap();
    assert_legacy_release_refused(&bench).await;
    std::fs::write(&payload, b"verified payloaD").unwrap();
    assert_legacy_release_refused(&bench).await;
    std::fs::write(payload, b"verified payload").unwrap();
    assert_legacy_release_replayed(&bench, &kept).await;
}

#[tokio::test]
async fn lfs_pointer_with_leading_whitespace_and_blank_lines_requires_its_payload() {
    payload_is_required_for_variant("blank").await;
}

#[tokio::test]
async fn a_1024_byte_lfs_pointer_requires_its_payload() {
    payload_is_required_for_variant("1024").await;
}
