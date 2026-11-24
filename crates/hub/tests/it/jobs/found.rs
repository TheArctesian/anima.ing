//! Found models through the hub's routes (task L1-22; K11 Sections 2.3 and 8; PULL-012, R-444):
//! `findModels` asks the machine for a `model_report`, `foundModels` lists the found models of
//! its result, `useFoundModel` makes a found copy and its `copy_adopt`, and `removeModelCopy`
//! of a found copy asks its machine for a `copy_delete` (PULL-009).

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::copies::{copies, report};
use super::{Work, model::model_claim};
use crate::daemon::harness::PVE_01;

const FOUND: &str = "{ foundModels { found tool name format fileCount sizeBytes usable
    place { kind id name } } }";
const FIND: &str = "mutation($deviceId: ID!) { findModels(deviceId: $deviceId) }";
const USE: &str = "mutation($deviceId: ID!, $found: String!) {
    useFoundModel(deviceId: $deviceId, found: $found) { id address origin state } }";
const REMOVE: &str = "mutation($id: ID!) { removeModelCopy(id: $id) { id state } }";

fn key(letter: char) -> String {
    letter.to_string().repeat(64)
}

fn cleanup() -> Value {
    json!({ "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" })
}

fn adopted(copy: &Value) -> Value {
    json!({ "result": { "outcome": "succeeded", "cause": null, "seconds": 2, "cleanup": cleanup(),
        "model": { "copy": copy, "copyState": "downloaded", "bytes": 100, "files": 1, "origin": "found",
            "fileHashes": [{ "sizeBytes": 100, "sha256": key('d') }] } } })
}

/// The result of a `model_report` that lists a GGUF model of Ollama, a safetensors model of
/// Hugging Face, and an entry with a key that is not 64 hexadecimal characters.
fn state() -> Value {
    let entry = |found: &str, tool: &str, name: &str, format: &str, usable: bool| {
        json!({ "found": found, "tool": tool, "name": name, "format": format,
            "files": 1, "bytes": 100, "usable": usable })
    };
    let found = [
        entry(&key('a'), "ollama", "qwen3:8b", "gguf", true),
        entry(
            &key('b'),
            "huggingface",
            "Qwen/Qwen3-8B/model.safetensors",
            "safetensors",
            false,
        ),
        entry("/home/me/.ollama", "ollama", "a path", "gguf", true),
    ];
    json!({ "result": { "outcome": "succeeded", "cause": null, "seconds": 1, "cleanup": cleanup(),
        "model": { "at": "2026-11-23T10:00:00Z", "settings": {}, "found": found } } })
}

async fn use_found(work: &Work, found: &str) -> Value {
    let variables = json!({ "deviceId": PVE_01, "found": found });
    work.daemons.operate(USE, variables).await
}

/// Claims the next item, checks its type, and reports it done with `last`.
async fn finish(work: &Work, token: &str, work_type: &str, last: &Value) -> Value {
    let (status, item) = work.claim(token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["type"], work_type);
    for (state, extra) in [
        ("preparing", json!({})),
        ("running", json!({})),
        ("finishing", json!({})),
        ("done", last.clone()),
    ] {
        let (status, body) = work.report(token, &report(&item, state, &extra)).await;
        assert_eq!(status, StatusCode::OK, "{state}: {body}");
    }
    item
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn pull_012_a_found_model_is_listed_and_used_where_it_is(pool: sqlx::PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let (status, _) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the hub learns the machine");

    let asked = work
        .daemons
        .operate(FIND, json!({ "deviceId": PVE_01 }))
        .await;
    assert!(asked["data"]["findModels"].is_string(), "{asked}");
    finish(&work, &token, "model_report", &state()).await;

    let listed = work.daemons.operate(FOUND, json!({})).await;
    let found = listed["data"]["foundModels"]
        .as_array()
        .expect("a list")
        .clone();
    let names: Vec<&str> = found
        .iter()
        .filter_map(|each| each["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["Qwen/Qwen3-8B/model.safetensors", "qwen3:8b"],
        "{listed}"
    );
    assert_eq!(found[1]["tool"], "OLLAMA");
    assert_eq!(found[1]["place"]["name"], "pve-01");

    let safetensors = use_found(&work, &key('b')).await;
    assert_eq!(
        safetensors["errors"][0]["extensions"]["code"], "VALIDATION",
        "{safetensors}"
    );
    let unknown = use_found(&work, &key('c')).await;
    assert_eq!(
        unknown["errors"][0]["extensions"]["code"], "NOT_FOUND",
        "{unknown}"
    );

    let used = use_found(&work, &key('a')).await;
    let copy = &used["data"]["useFoundModel"];
    assert_eq!(
        (&copy["address"], &copy["origin"], &copy["state"]),
        (&json!("qwen3:8b"), &json!("FOUND"), &json!("QUEUED")),
        "{used}"
    );
    let listed = work.daemons.operate(FOUND, json!({})).await;
    assert_eq!(
        listed["data"]["foundModels"].as_array().map(Vec::len),
        Some(1),
        "a used model is not listed"
    );

    let item = finish(&work, &token, "copy_adopt", &adopted(&copy["id"])).await;
    assert_eq!(
        item["parameters"],
        json!({ "copy": copy["id"], "found": key('a') })
    );
    assert_eq!(copies(&work).await[0]["state"], "DOWNLOADED");

    let again = use_found(&work, &key('a')).await;
    assert!(
        again["errors"][0]["extensions"]["code"].is_string(),
        "a used model is not used twice: {again}"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn pull_009_the_removal_of_a_found_copy_deletes_its_record_on_its_machine(
    pool: sqlx::PgPool,
) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let (status, _) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the hub learns the machine");
    work.daemons
        .operate(FIND, json!({ "deviceId": PVE_01 }))
        .await;
    finish(&work, &token, "model_report", &state()).await;
    let used = use_found(&work, &key('a')).await;
    let copy = used["data"]["useFoundModel"]["id"].clone();
    finish(&work, &token, "copy_adopt", &adopted(&copy)).await;

    let removed = work.daemons.operate(REMOVE, json!({ "id": copy })).await;
    assert_eq!(
        removed["data"]["removeModelCopy"]["state"], "REMOVING",
        "{removed}"
    );
    // The daemon sends no `model` for a `copy_delete` (it deletes only its record of the copy).
    let done = json!({ "result": { "outcome": "succeeded", "cause": null, "seconds": 0, "cleanup": cleanup() } });
    let item = finish(&work, &token, "copy_delete", &done).await;
    assert_eq!(item["parameters"], json!({ "copy": copy }));
    assert!(
        copies(&work).await.is_empty(),
        "a removed copy is not listed"
    );

    let listed = work.daemons.operate(FOUND, json!({})).await;
    let names: Vec<&str> = listed["data"]["foundModels"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|each| each["name"].as_str())
        .collect();
    assert!(
        names.contains(&"qwen3:8b"),
        "the model is found again when no copy uses it: {listed}"
    );
    let again = use_found(&work, &key('a')).await;
    assert_eq!(again["data"]["useFoundModel"]["state"], "QUEUED", "{again}");
}
