use super::*;

#[tokio::test]
async fn external_lfs_attributes_cannot_hide_same_size_corruption_in_the_project_cache() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let external = bench.home.with_file_name("legacy-attributes");
    std::fs::write(&external, "example.bin filter=lfs\n").unwrap();
    bench
        .git_in(
            &bench.project,
            &["config", "core.attributesFile", external.to_str().unwrap()],
        )
        .await;
    let (kept, project_cache) = pointer(&bench, false, "plain").await;
    assert_eq!(
        bench
            .git_in(
                &bench.project,
                &["check-attr", "filter", "--", "example.bin"]
            )
            .await,
        "example.bin: filter: lfs"
    );
    let common = bench.project.join(".git");
    let private = own_of(&bench).join(project_cache.strip_prefix(common).unwrap());
    let correct = b"verified payload";
    std::fs::create_dir_all(private.parent().unwrap()).unwrap();
    std::fs::write(&private, correct).unwrap();
    std::fs::write(&project_cache, b"verified payloaD").unwrap();

    assert_legacy_release_refused(&bench).await;
    assert!(folder.is_dir());
    assert_eq!(std::fs::read(private).unwrap(), correct);
    std::fs::write(project_cache, correct).unwrap();
    assert_legacy_release_replayed(&bench, &kept).await;
}

async fn ordinary_blob_survives(data: &[u8]) {
    let bench = Bench::new().await;
    bench.worktree().await;
    std::fs::write(bench.project.join(".gitattributes"), "raw.bin filter=lfs\n").unwrap();
    std::fs::write(bench.project.join("raw.bin"), data).unwrap();
    let (kept, _) = saved(&bench).await;
    assert_legacy_release_replayed(&bench, &kept).await;
    assert_eq!(
        bench
            .git_in(
                &bench.project,
                &["cat-file", "-s", &format!("{kept}:raw.bin")]
            )
            .await,
        data.len().to_string()
    );
}

#[tokio::test]
async fn ordinary_binary_blob_with_lfs_attribute_is_recoverable() {
    ordinary_blob_survives(b"\xff\x00fixture\nversion https://git-lfs.github.com/spec/v1\n").await;
}

#[tokio::test]
async fn ordinary_blob_larger_than_the_git_output_cap_is_recoverable() {
    ordinary_blob_survives(&vec![b'x'; crate::threads::git::run::MAX_OUTPUT + 1]).await;
}

#[tokio::test]
async fn a_malformed_pointer_candidate_remains_a_refusal() {
    let bench = Bench::new().await;
    bench.worktree().await;
    std::fs::write(bench.project.join(".gitattributes"), "raw.bin filter=lfs\n").unwrap();
    std::fs::write(
        bench.project.join("raw.bin"),
        b"version https://git-lfs.github.com/spec/v1\noid sha256:bad\nsize 50\n",
    )
    .unwrap();
    saved(&bench).await;
    assert_legacy_release_refused(&bench).await;
}

#[tokio::test]
async fn a_custom_legacy_cache_with_unproved_destination_prevents_removal() {
    let bench = Bench::new().await;
    let original = bench.worktree().await;
    let attributes = bench.home.with_file_name("external-legacy-lfs");
    std::fs::write(&attributes, "example.bin filter=lfs\n").unwrap();
    bench
        .git_in(
            &bench.project,
            &[
                "config",
                "core.attributesFile",
                attributes.to_str().unwrap(),
            ],
        )
        .await;
    let (_, project_cache) = pointer(&bench, false, "plain").await;
    bench
        .git_in(&bench.project, &["config", "lfs.storage", "custom-lfs"])
        .await;
    bench
        .git_own(&["config", "lfs.storage", "custom-lfs"])
        .await;
    let suffix = project_cache
        .strip_prefix(bench.project.join(".git/lfs"))
        .unwrap();
    let private = own_of(&bench).join("custom-lfs").join(suffix);
    std::fs::create_dir_all(private.parent().unwrap()).unwrap();
    std::fs::write(&private, b"verified payload").unwrap();

    assert_legacy_release_refused(&bench).await;
    bench.git_own(&["config", "--unset", "lfs.storage"]).await;
    assert_legacy_release_refused(&bench).await;
    assert!(original.is_dir());
    assert_eq!(std::fs::read(private).unwrap(), b"verified payload");
    assert!(!own_of(&bench).join("lfs").exists());
    assert!(!bench.project.join(".git/custom-lfs").join(suffix).exists());
}

#[tokio::test]
async fn an_explicit_empty_pointer_needs_no_lfs_payload() {
    let bench = Bench::new().await;
    bench.worktree().await;
    std::fs::write(
        bench.project.join(".gitattributes"),
        "empty.bin filter=lfs\n",
    )
    .unwrap();
    let hash = crate::model::fetch::hex(&sha2::Sha256::digest(b""));
    std::fs::write(
        bench.project.join("empty.bin"),
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{hash}\nsize 0\n"),
    )
    .unwrap();
    let (kept, _) = saved(&bench).await;
    assert_legacy_release_replayed(&bench, &kept).await;
    assert!(!bench.project.join(".git/lfs/objects").exists());
}

#[tokio::test]
async fn a_legacy_pointer_without_attribute_provenance_requires_payload_proof() {
    for absent in [false, true] {
        let bench = Bench::new().await;
        let original = bench.worktree().await;
        let (kept, project_cache) = pointer(&bench, false, "plain").await;
        let cache = own_of(&bench)
            .parent()
            .unwrap()
            .join("external-review-cache");
        std::fs::create_dir_all(&cache).unwrap();
        let payload = cache.join(project_cache.file_name().unwrap());
        std::fs::write(&payload, b"verified payload").unwrap();
        if absent {
            std::fs::remove_dir_all(&original).unwrap();
        }
        assert_legacy_release_refused(&bench).await;
        assert_eq!(std::fs::read(&payload).unwrap(), b"verified payload");
        std::fs::write(&project_cache, b"verified payload").unwrap();
        assert_legacy_release_replayed(&bench, &kept).await;
    }
}
