use anima_daemon::hub::{HubClient, TIMEOUT};
use daemon_core::wire::DaemonToken;
use serde_json::{Value, json};
use uuid::Uuid;

use super::found::{fixture, key_of, ollama, report};
use super::kit::{assert_k10_states, node};
use crate::fake_hub::{FakeHub, Reply};

pub(super) fn token() -> DaemonToken {
    DaemonToken::from_random([7; 32])
}

pub(super) fn item(work_type: &str, parameters: &serde_json::Value) -> String {
    json!({
        "id": "0192f6aa-0000-7000-8000-000000000009", "type": work_type, "kind": "model", "attempt": 1,
        "lease": { "until": "2026-11-20T10:01:30Z" },
        "limits": { "seconds": 600, "idleSeconds": 60, "memoryBytes": 1, "diskBytes": 1, "orphanSeconds": 60 },
        "project": null, "secrets": [], "parameters": parameters, "actor": "User",
    })
    .to_string()
}

#[tokio::test]
async fn the_claim_names_only_the_built_model_types() {
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    assert_eq!(node.cycle(&client, &token()).await, Ok(None));
    let claim = &hub.seen("claim")[0].body;
    assert_eq!(claim["kinds"], json!(["model"]));
    assert_eq!(
        claim["types"],
        json!([
            "model_report",
            "model_drain",
            "runtime_install",
            "model_pull",
            "copy_delete",
            "copy_adopt",
            "server_start",
            "server_stop",
            "model_load",
            "model_unload",
            "model_keep"
        ])
    );
    assert!(hub.seen("report").is_empty());
}

#[tokio::test]
async fn work_outside_the_list_is_refused() {
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    for (work_type, cause) in [
        ("build", "type_unknown"),
        ("shell", "type_unknown"),
        ("model_measure", "type_unknown"),
    ] {
        let parameters = json!({ "server": "0192f6aa-0000-7000-8000-000000000001", "copy": "0192f6aa-0000-7000-8000-000000000002", "probe": "text_short" });
        hub.queue_claim(Reply::json(200, &item(work_type, &parameters)));
        let before = hub.seen("report").len();
        node.cycle(&client, &token()).await.expect("a cycle");
        let reports = hub.seen("report").split_off(before);
        assert_k10_states(reports.iter().map(|report| &report.body));
        assert_eq!(reports.len(), 1, "{work_type}");
        let report = &reports[0].body;
        assert_eq!(report["state"], json!("refused"), "{work_type}");
        assert_eq!(report["result"]["cause"], json!(cause), "{work_type}");
    }
}

#[tokio::test]
async fn a_model_report_gives_the_state_and_the_settings() {
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    hub.queue_claim(Reply::json(200, &item("model_report", &json!({}))));
    node.cycle(&client, &token()).await.expect("a cycle");
    let reports = hub.seen("report");
    assert_k10_states(reports.iter().map(|report| &report.body));
    let done = reports.last().expect("a report");
    let model = &done.body["result"]["model"];
    assert_eq!(done.body["state"], json!("done"));
    assert_eq!(model["settings"]["sandbox"], json!(false));
    assert_eq!(model["installed"], json!([]));
    assert_eq!(done.bearer.as_deref(), Some(token().expose()));
}

#[tokio::test]
async fn an_owned_flag_and_an_agent_outside_agent_types_are_refused() {
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    let start = json!({ "server": "0192f6aa-0000-7000-8000-000000000001", "runtime": "llama_server", "version": "b8100", "backend": "cpu", "settings": { "host": "0.0.0.0" } });
    hub.queue_claim(Reply::json(200, &item("server_start", &start)));
    node.cycle(&client, &token()).await.expect("a cycle");
    assert_eq!(
        hub.seen("report")[0].body["result"]["cause"],
        json!("flag_owned")
    );
    node.settings.agent_types = vec![daemon_core::wire::WorkType::ServerStop];
    let mut agent: serde_json::Value =
        serde_json::from_str(&item("model_report", &json!({}))).expect("json");
    agent["actor"] = json!("Agent");
    hub.queue_claim(Reply::json(200, &agent.to_string()));
    node.cycle(&client, &token()).await.expect("a cycle");
    assert_eq!(
        hub.seen("report")[1].body["result"]["cause"],
        json!("actor_not_permitted")
    );
}

#[tokio::test]
async fn cap_013_a_model_drain_succeeds_within_its_seconds_and_stops_no_server() {
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    hub.queue_claim(Reply::json(
        200,
        &item("model_drain", &json!({ "seconds": 60 })),
    ));
    let start = std::time::Instant::now();
    node.cycle(&client, &token()).await.expect("a cycle");
    assert!(start.elapsed() <= std::time::Duration::from_secs(60));
    let reports = hub.seen("report");
    assert_k10_states(reports.iter().map(|report| &report.body));
    assert_eq!(
        reports.last().expect("a report").body["state"],
        json!("done")
    );
}

#[tokio::test]
async fn pull_009_a_pulled_copy_is_downloaded_and_a_copy_delete_removes_it() {
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    let copy = "0192f6aa-0000-7000-8000-000000000003";
    let folder = node.folders.models.join("copies").join(copy);
    std::fs::create_dir_all(&folder).expect("the copy");
    let file =
        json!({ "name": "q4/Qwen3-8B-Q4_K_M.gguf", "sizeBytes": 5, "sha256": "b".repeat(64) });
    let source = json!({ "organisation": "unsloth", "repository": "Qwen3-8B-GGUF", "revision": "a".repeat(40) });
    let pull = json!({ "copy": copy, "variant": copy, "source": source, "format": "gguf", "files": [file], "gated": false });
    hub.queue_claim(Reply::json(200, &item("model_pull", &pull)));
    node.cycle(&client, &token()).await.expect("a cycle");
    let pulled = hub.seen("report");
    hub.queue_claim(Reply::json(
        200,
        &item("copy_delete", &json!({ "copy": copy })),
    ));
    node.cycle(&client, &token()).await.expect("a cycle");
    let deleted = hub.seen("report").split_off(pulled.len());

    assert_k10_states(pulled.iter().map(|report| &report.body));
    let model = json!({ "copy": copy, "copyState": "downloaded", "bytes": 5, "files": 1 });
    assert_eq!(
        pulled.last().expect("a report").body["result"]["model"],
        model
    );
    assert_k10_states(deleted.iter().map(|report| &report.body));
    assert!(!folder.exists());
}

#[tokio::test]
async fn a_long_adopt_keeps_its_lease_with_a_report_at_each_interval() {
    let base = tempfile::tempdir().expect("temporary");
    let (mut node, roots) = fixture(base.path());
    ollama(&roots[0].path, &vec![7; 32 << 20]);
    let found = report(&mut node).await;
    let adopt = json!({ "copy": Uuid::now_v7(), "found": key_of(&found, "qwen3:8b") });
    node.report_every = std::time::Duration::from_millis(2);
    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    hub.queue_claim(Reply::json(200, &item("copy_adopt", &adopt)));
    node.cycle(&client, &token()).await.expect("a cycle");

    let reports: Vec<Value> = hub
        .seen("report")
        .into_iter()
        .map(|seen| seen.body)
        .collect();
    assert_k10_states(&reports);
    let running = reports
        .iter()
        .filter(|body| body["state"] == "running")
        .count();
    assert!(running >= 2, "no report while the hash ran: {reports:?}");
    assert!(reports.iter().all(|body| body.get("progress").is_none()));
    assert_eq!(reports.last().expect("a report")["state"], "done");
}

/// K11 Section 11.1: a keep is for a copy on the node, the node writes it to a private file, and a load needs a
/// server of the node that is ready. A load on a real router is HT-NEW-modelloading.
#[tokio::test]
async fn a_keep_is_written_for_a_copy_of_the_node_and_a_load_needs_a_ready_server() {
    use std::os::unix::fs::PermissionsExt as _;

    let hub = FakeHub::start().await;
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    let root = tempfile::tempdir().expect("a folder");
    let mut node = node(root.path());
    let copy = "0192f6aa-0000-7000-8000-000000000002";
    let keep = json!({ "copy": copy, "keep": { "idleSeconds": 120, "pinned": true } });
    let load = json!({
        "server": "0192f6aa-0000-7000-8000-000000000001", "copy": copy, "settings": {},
        "keep": { "idleSeconds": 120, "pinned": false },
    });
    let mut run = async |work_type: &str, parameters: &Value| {
        hub.queue_claim(Reply::json(200, &item(work_type, parameters)));
        node.cycle(&client, &token()).await.expect("a cycle");
        let last = hub.seen("report").last().expect("a report").body.clone();
        (last["state"].clone(), last["result"]["cause"].clone())
    };
    assert_eq!(run("model_keep", &keep).await.1, json!("copy_unknown"));
    assert_eq!(run("model_load", &load).await.1, json!("copy_unknown"));

    std::fs::create_dir_all(root.path().join("models/copies").join(copy)).expect("a copy");
    assert_eq!(run("model_keep", &keep).await.0, json!("done"));
    let file = root
        .path()
        .join("model-state/keep")
        .join(format!("{copy}.json"));
    let written: Value =
        serde_json::from_slice(&std::fs::read(&file).expect("the keep file")).expect("json");
    assert_eq!(written["keep"], keep["keep"]);
    let mode = std::fs::metadata(&file)
        .expect("the file")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(
        run("model_load", &load).await.1,
        json!("copy_unknown"),
        "no server is ready"
    );
}
