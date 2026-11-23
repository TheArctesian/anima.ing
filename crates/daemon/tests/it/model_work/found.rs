//! Found models (`contracts/compute/model-work.md` Sections 2.3 and 10.2; R-444): the walk of a
//! folder of each tool, `copy_adopt`, the check at a load, and that nothing in those folders is
//! written.

use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use anima_daemon::model::cycle::Node;
use anima_daemon::model::found;
use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::found::{Root, Tool};
use daemon_core::model_work::names::Actor;
use daemon_core::model_work::order::{Order, Parameters};
use daemon_core::wire::WorkType;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use super::kit::node;

pub(super) const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

pub(super) fn sha(bytes: &[u8]) -> String {
    anima_daemon::model::fetch::hex(&Sha256::digest(bytes))
}

pub(super) fn put(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folders");
    std::fs::write(path, bytes).expect("written");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).expect("mode");
}

pub(super) fn ollama(root: &Path, model: &[u8]) {
    let digest = sha(model);
    let manifest = format!(
        r#"{{"schemaVersion":2,"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{digest}","size":{}}}]}}"#,
        model.len()
    );
    put(
        &root.join("manifests/registry.ollama.ai/library/qwen3/8b"),
        manifest.as_bytes(),
    );
    put(&root.join(format!("blobs/sha256-{digest}")), model);
}

fn huggingface(root: &Path, model: &[u8]) {
    let repository = root.join("models--unsloth--Qwen3-8B-GGUF");
    let blob = sha(model);
    put(&repository.join("blobs").join(&blob), model);
    let snapshot = repository.join("snapshots").join(COMMIT);
    std::fs::create_dir_all(&snapshot).expect("snapshot");
    std::os::unix::fs::symlink(
        format!("../../blobs/{blob}"),
        snapshot.join("Qwen3-8B-Q4_K_M.gguf"),
    )
    .expect("link");
}

/// A folder of each tool below `base`, with the node that reads them.
pub(super) fn fixture(base: &Path) -> (Node, Vec<Root>) {
    let roots = vec![
        Root {
            tool: Tool::Ollama,
            path: base.join("ollama"),
        },
        Root {
            tool: Tool::Huggingface,
            path: base.join("hf"),
        },
        Root {
            tool: Tool::LmStudio,
            path: base.join("lmstudio"),
        },
        Root {
            tool: Tool::Folder,
            path: base.join("mine"),
        },
    ];
    ollama(&roots[0].path, b"GGUF ollama");
    huggingface(&roots[1].path, b"GGUF huggingface");
    put(
        &roots[2]
            .path
            .join("lmstudio-community/Qwen3-GGUF/Qwen3-4B.gguf"),
        b"GGUF lm studio",
    );
    put(&roots[3].path.join("a/b/near.gguf"), b"GGUF near");
    put(&roots[3].path.join("a/b/c/deep.gguf"), b"GGUF too deep");
    let mut node = node(&base.join("node"));
    node.tool_folders = roots[..3].to_vec();
    node.settings.model_folders = vec![roots[3].path.to_str().expect("text").to_owned()];
    (node, roots)
}

fn order(work_type: WorkType, parameters: Parameters) -> Order {
    Order {
        id: Uuid::nil(),
        attempt: 1,
        work_type,
        actor: Actor::User,
        parameters,
    }
}

pub(super) async fn report(node: &mut Node) -> Vec<Value> {
    let state = node
        .run(&order(WorkType::ModelReport, Parameters::Report))
        .await;
    let state = state.expect("report").expect("state");
    state["found"].as_array().expect("found").clone()
}

pub(super) fn key_of(found: &[Value], name: &str) -> String {
    let entry = found
        .iter()
        .find(|entry| entry["name"] == name)
        .expect(name);
    entry["found"].as_str().expect("key").to_owned()
}

pub(super) async fn adopt(
    node: &mut Node,
    copy: Uuid,
    found: String,
) -> Result<Option<Value>, Cause> {
    node.run(&order(
        WorkType::CopyAdopt,
        Parameters::Adopt { copy, found },
    ))
    .await
}

/// Each path below `root` with its size, mode and modification time.
pub(super) fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u64, u32, i64)> {
    let mut seen = BTreeMap::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(path) = queue.pop() {
        let meta = std::fs::symlink_metadata(&path).expect("metadata");
        if meta.is_dir() {
            queue.extend(
                std::fs::read_dir(&path)
                    .expect("read")
                    .map(|entry| entry.expect("entry").path()),
            );
        }
        seen.insert(
            path,
            (
                meta.len(),
                meta.mode(),
                meta.mtime_nsec() + meta.mtime() * 1_000_000_000,
            ),
        );
    }
    seen
}

#[tokio::test]
async fn the_report_lists_the_models_of_each_tool_and_no_deeper_file() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, _) = fixture(base.path());
    let found = report(&mut node).await;
    let mut names: Vec<&str> = found
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "a/b/near.gguf",
            "lmstudio-community/Qwen3-GGUF/Qwen3-4B.gguf",
            "qwen3:8b",
            "unsloth/Qwen3-8B-GGUF/Qwen3-8B-Q4_K_M.gguf"
        ]
    );
    assert!(
        found
            .iter()
            .all(|entry| entry["usable"] == true && entry["format"] == "gguf")
    );
    assert!(found.iter().all(|entry| {
        !entry
            .to_string()
            .contains(base.path().to_str().expect("text"))
    }));
    node.settings.find_models = false;
    assert_eq!(report(&mut node).await, Vec::<Value>::new());
}

#[tokio::test]
async fn an_adopted_copy_is_checked_again_and_a_changed_file_is_refused() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    let found = report(&mut node).await;
    let copy = Uuid::now_v7();
    let model = adopt(&mut node, copy, key_of(&found, "qwen3:8b")).await;
    let model = model.expect("adopted").expect("model");
    assert_eq!(model["origin"], "found");
    assert_eq!(model["fileHashes"][0]["sha256"], sha(b"GGUF ollama"));
    let state = node.folders.state.clone();
    assert_eq!(
        found::check(&node.found_roots(), &state, copy).map(|fds| fds.len()),
        Ok(1)
    );

    let blob = roots[0]
        .path
        .join(format!("blobs/sha256-{}", sha(b"GGUF ollama")));
    put(&blob, b"GGUF 0llama");
    assert_eq!(
        found::check(&node.found_roots(), &state, copy).err(),
        Some(Cause::CopyChanged)
    );
    std::fs::remove_file(&blob).expect("removed");
    assert_eq!(
        found::check(&node.found_roots(), &state, copy).err(),
        Some(Cause::CopyMissing)
    );
    assert_eq!(
        adopt(&mut node, Uuid::now_v7(), "a".repeat(64)).await,
        Err(Cause::FoundUnknown)
    );
}

#[tokio::test]
async fn a_blob_that_is_not_its_name_is_refused() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    let found = report(&mut node).await;
    let name = "unsloth/Qwen3-8B-GGUF/Qwen3-8B-Q4_K_M.gguf";
    let blob = roots[1]
        .path
        .join("models--unsloth--Qwen3-8B-GGUF/blobs")
        .join(sha(b"GGUF huggingface"));
    put(&blob, b"GGUF hugginGface");
    assert_eq!(
        adopt(&mut node, Uuid::now_v7(), key_of(&found, name)).await,
        Err(Cause::Checksum)
    );
}

#[tokio::test]
async fn nothing_in_a_tool_folder_is_written_and_a_delete_removes_only_the_record() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    let before: Vec<_> = roots.iter().map(|root| snapshot(&root.path)).collect();
    let found = report(&mut node).await;
    let mut copies = Vec::new();
    for entry in &found {
        let copy = Uuid::now_v7();
        let key = entry["found"].as_str().expect("key").to_owned();
        adopt(&mut node, copy, key).await.expect("adopted");
        copies.push(copy);
    }
    for copy in &copies {
        let delete = order(WorkType::CopyDelete, Parameters::CopyDelete { copy: *copy });
        assert_eq!(node.run(&delete).await, Ok(None));
        let roots = node.found_roots();
        assert_eq!(
            found::check(&roots, &node.folders.state, *copy).err(),
            Some(Cause::CopyUnknown)
        );
    }
    let after: Vec<_> = roots.iter().map(|root| snapshot(&root.path)).collect();
    assert_eq!(before, after);
}

#[tokio::test]
async fn a_link_out_of_a_folder_and_a_file_that_others_write_are_not_listed() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    put(
        &base.path().join("elsewhere/secret.gguf"),
        b"GGUF elsewhere",
    );
    let publisher = roots[2].path.join("lmstudio-community/Qwen3-GGUF");
    std::os::unix::fs::symlink(
        base.path().join("elsewhere/secret.gguf"),
        publisher.join("link.gguf"),
    )
    .expect("link");
    let shared = roots[3].path.join("a/shared.gguf");
    put(&shared, b"GGUF shared");
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o666)).expect("mode");
    let found = report(&mut node).await;
    let names: Vec<&str> = found
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert!(
        !names
            .iter()
            .any(|name| name.contains("link") || name.contains("shared")),
        "{names:?}"
    );
    assert_eq!(names.len(), 4);
}
