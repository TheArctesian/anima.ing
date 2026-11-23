//! A blob of the Hugging Face cache in the shared store of Unsloth Studio, and its projector
//! (`contracts/compute/model-work.md` Section 2.3; R-443).

use std::path::Path;

use anima_daemon::model::found;
use uuid::Uuid;

use super::found::{COMMIT, adopt, fixture, key_of, put, report, sha, snapshot};

/// A repository of the cache whose blob links to the shared store `blobs/<xx>/` of the cache, as
/// the downloader of Unsloth Studio keeps it, with the one projector of its snapshot.
fn stored(root: &Path, model: &[u8], projector: &[u8]) {
    let repository = root.join("models--unsloth--Gemma-GGUF");
    let snapshot = repository.join("snapshots").join(COMMIT);
    std::fs::create_dir_all(&snapshot).expect("snapshot");
    for (name, bytes) in [("Gemma-Q4.gguf", model), ("mmproj-F16.gguf", projector)] {
        let blob = sha(bytes);
        let store = sha(&[bytes, b"store"].concat());
        put(&root.join("blobs").join(&store[..2]).join(&store), bytes);
        std::fs::create_dir_all(repository.join("blobs")).expect("blobs");
        std::os::unix::fs::symlink(
            format!("../../blobs/{}/{store}", &store[..2]),
            repository.join("blobs").join(&blob),
        )
        .expect("store link");
        std::os::unix::fs::symlink(format!("../../blobs/{blob}"), snapshot.join(name))
            .expect("link");
    }
}

#[tokio::test]
async fn a_blob_in_the_shared_store_of_the_cache_is_found_with_its_projector_and_adopted() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    stored(&roots[1].path, b"GGUF gemma", b"GGUF projector");
    let before = snapshot(&roots[1].path);
    let found = report(&mut node).await;
    let name = "unsloth/Gemma-GGUF/Gemma-Q4.gguf";
    let entry = found
        .iter()
        .find(|entry| entry["name"] == name)
        .expect(name);
    assert_eq!(entry["files"], 2);
    assert!(!found.iter().any(|entry| {
        entry["name"]
            .as_str()
            .is_some_and(|name| name.contains("mmproj"))
    }));
    let copy = Uuid::now_v7();
    let model = adopt(&mut node, copy, key_of(&found, name)).await;
    let model = model.expect("adopted").expect("model");
    assert_eq!(model["fileHashes"][0]["sha256"], sha(b"GGUF gemma"));
    assert_eq!(model["fileHashes"][1]["sha256"], sha(b"GGUF projector"));
    let state = node.folders.state.clone();
    let fds = found::check(&node.found_roots(), &state, copy).map(|fds| fds.len());
    assert_eq!(fds, Ok(2));
    assert_eq!(before, snapshot(&roots[1].path));
}

#[tokio::test]
async fn a_blob_link_out_of_the_shared_store_is_not_followed() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    let repository = roots[1].path.join("models--unsloth--Away-GGUF");
    let blob = sha(b"GGUF away");
    put(&base.path().join("elsewhere").join(&blob), b"GGUF away");
    std::fs::create_dir_all(repository.join("blobs")).expect("blobs");
    std::os::unix::fs::symlink(
        format!("../../../elsewhere/{blob}"),
        repository.join("blobs").join(&blob),
    )
    .expect("store link");
    let snapshot = repository.join("snapshots").join(COMMIT);
    std::fs::create_dir_all(&snapshot).expect("snapshot");
    std::os::unix::fs::symlink(format!("../../blobs/{blob}"), snapshot.join("Away.gguf"))
        .expect("link");
    let found = report(&mut node).await;
    assert!(!found.iter().any(|entry| {
        entry["name"]
            .as_str()
            .is_some_and(|name| name.contains("Away"))
    }));
}
